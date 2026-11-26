//! Construction of a [`Spec`] from JSON, and why a version declaration is rejected.
//!
//! A spec compiles as a pipeline: `STEPS` runs in order over a `Build`, one
//! step per section or structural check, each reading what the steps before
//! it built.

use std::collections::BTreeMap;
use std::fmt;

use super::balance::compile_balancing;
use super::build_loops::{check_control_fields, check_cycles, compile_loop, held_segments};
use super::build_occurrences::{check_trigger, compile_occurrences};
use super::compile::compile_tables;
use super::error::SpecError;
use super::model::{SegmentModel, SpecModel, VersionModel};
use super::places::place_columns;
use super::segments::compile_elements;
use super::summaries::{compile_default_table, compile_summaries};
use crate::spec::{
    DeclaredVersion, LoopId, OccurrenceIndex, ReadIndex, SegmentDef, Spec, Trigger, render_trigger,
};

/// A spec while it compiles, and the model every step reads.
struct Build<'m> {
    spec: Spec,
    /// The spec as its model reads it, keys in name order.
    model: &'m SpecModel,
}

type Step = fn(&mut Build<'_>) -> Result<(), SpecError>;

/// The compile steps, in the order they run.
const STEPS: [Step; 14] = [
    compile_loop_headers,
    link_loops,
    compile_segment_section,
    check_control_fields_step,
    compile_loop_occurrences,
    index_occurrences,
    compile_version_section,
    check_trigger_ambiguity,
    compile_table_section,
    compile_place_section,
    index_reads,
    compile_balancing_section,
    compile_summary_section,
    compile_default_table_section,
];

impl Spec {
    /// Loads and validates a spec from JSON text.
    pub fn from_json(json: &str) -> Result<Spec, SpecError> {
        SpecModel::from_json(json).and_then(|model| model.compile())
    }

    /// Compiles `model`.
    pub(in crate::spec_load) fn build(model: &SpecModel) -> Result<Spec, SpecError> {
        let mut build = Build::new(model)?;
        STEPS.iter().try_for_each(|step| step(&mut build))?;
        Ok(build.spec)
    }

    fn check_ambiguity(&self) -> Result<(), SpecError> {
        let groups = std::iter::once(self.roots.as_slice())
            .chain(self.loops.iter().map(|def| def.children.as_slice()));
        for siblings in groups {
            for (i, &first) in siblings.iter().enumerate() {
                for &second in &siblings[i + 1..] {
                    let trigger = &self.loops[first.0].trigger;
                    let other = &self.loops[second.0].trigger;
                    if trigger.segment != other.segment {
                        continue;
                    }
                    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
                    let parent = self.loops[first.0]
                        .parent
                        .map(|parent| self.loops[parent.0].name.clone());
                    if trigger == other {
                        return Err(SpecError::AmbiguousTrigger {
                            first: self.loops[first.0].name.clone(),
                            second: self.loops[second.0].name.clone(),
                            parent,
                            segment: text(&trigger.segment),
                            conditions: trigger
                                .conditions
                                .iter()
                                .map(|(position, value)| (*position, text(value)))
                                .collect(),
                        });
                    }
                    // Siblings are told apart when a shared position requires
                    // different values, or when one trigger's conditions
                    // contain the other's: the engine then prefers the one
                    // with more conditions and the other is the catch-all.
                    let excluded = trigger.conditions.iter().any(|(position, value)| {
                        other
                            .conditions
                            .iter()
                            .any(|(p, v)| p == position && v != value)
                    });
                    let contains = |big: &Trigger, small: &Trigger| {
                        small
                            .conditions
                            .iter()
                            .all(|condition| big.conditions.contains(condition))
                    };
                    let nested = contains(trigger, other) || contains(other, trigger);
                    if !excluded && !nested {
                        return Err(SpecError::OverlappingTriggers {
                            parent,
                            a: self.loops[first.0].name.clone(),
                            b: self.loops[second.0].name.clone(),
                            conditions_a: render_trigger(trigger),
                            conditions_b: render_trigger(other),
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

impl<'m> Build<'m> {
    /// The spec's name and an empty structure, or why the spec declares no loops.
    fn new(model: &'m SpecModel) -> Result<Build<'m>, SpecError> {
        if model.loops.is_empty() {
            return Err(SpecError::NoLoops {
                spec_name: model.name.clone(),
            });
        }
        Ok(Build {
            spec: Spec {
                name: model.name.clone(),
                loops: Vec::new(),
                occurrence_index: OccurrenceIndex::default(),
                roots: Vec::new(),
                segments: BTreeMap::new(),
                tables: Vec::new(),
                reads: ReadIndex::default(),
                balancing: Vec::new(),
                summaries: Vec::new(),
                default_table: None,
                version: None,
            },
            model,
        })
    }
}

fn compile_loop_headers(build: &mut Build<'_>) -> Result<(), SpecError> {
    let ids: BTreeMap<&str, LoopId> = build
        .model
        .loops
        .keys()
        .enumerate()
        .map(|(index, name)| (name.as_str(), LoopId(index)))
        .collect();
    let loops = build
        .model
        .loops
        .iter()
        .map(|(name, def)| compile_loop(name, def, &ids))
        .collect::<Result<Vec<_>, _>>()?;
    check_cycles(&loops)?;
    build.spec.loops = loops;
    Ok(())
}

/// Gives each loop its children and the spec its top-level loops.
fn link_loops(build: &mut Build<'_>) -> Result<(), SpecError> {
    let spec = &mut build.spec;
    let parents: Vec<Option<LoopId>> = spec.loops.iter().map(|def| def.parent).collect();
    for (index, parent) in parents.into_iter().enumerate() {
        match parent {
            Some(parent) => spec.loops[parent.0].children.push(LoopId(index)),
            None => spec.roots.push(LoopId(index)),
        }
    }
    Ok(())
}

fn compile_segment_section(build: &mut Build<'_>) -> Result<(), SpecError> {
    build.spec.segments = build
        .model
        .segments
        .iter()
        .map(|(id, def)| compile_segment(id, def))
        .collect::<Result<_, _>>()?;
    Ok(())
}

fn compile_segment(id: &str, def: &SegmentModel) -> Result<(Vec<u8>, SegmentDef), SpecError> {
    if id.is_empty() {
        return Err(SpecError::EmptySegmentId {
            loop_name: None,
            key: "segments.\"\"".into(),
        });
    }
    let elements = compile_elements(id, &def.elements, None)?;
    Ok((id.as_bytes().to_vec(), SegmentDef { elements }))
}

fn compile_loop_occurrences(build: &mut Build<'_>) -> Result<(), SpecError> {
    let spec = &mut build.spec;
    for ((name, def), compiled) in build.model.loops.iter().zip(spec.loops.iter_mut()) {
        compiled.occurrences = compile_occurrences(name, &def.occurrences, &spec.segments)?;
        check_trigger(name, &compiled.occurrences, &compiled.trigger)?;
        compiled.segments = held_segments(compiled);
    }
    Ok(())
}

fn index_occurrences(build: &mut Build<'_>) -> Result<(), SpecError> {
    build.spec.occurrence_index = OccurrenceIndex::build(&build.spec.loops);
    Ok(())
}

fn compile_version_section(build: &mut Build<'_>) -> Result<(), SpecError> {
    build.spec.version = build
        .model
        .version
        .as_ref()
        .map(|raw| compile_version(raw, &build.model.segments))
        .transpose()?;
    Ok(())
}

fn check_control_fields_step(build: &mut Build<'_>) -> Result<(), SpecError> {
    check_control_fields(&build.spec)
}

fn check_trigger_ambiguity(build: &mut Build<'_>) -> Result<(), SpecError> {
    build.spec.check_ambiguity()
}

fn compile_table_section(build: &mut Build<'_>) -> Result<(), SpecError> {
    let spec = &build.spec;
    build.spec.tables = compile_tables(spec, &build.model.tables)?;
    Ok(())
}

fn compile_place_section(build: &mut Build<'_>) -> Result<(), SpecError> {
    let places = place_columns(&build.spec);
    for (def, places) in build.spec.tables.iter_mut().zip(places) {
        def.places = places;
    }
    Ok(())
}

fn index_reads(build: &mut Build<'_>) -> Result<(), SpecError> {
    build.spec.reads = ReadIndex::build(&build.spec);
    Ok(())
}

fn compile_balancing_section(build: &mut Build<'_>) -> Result<(), SpecError> {
    let spec = &build.spec;
    build.spec.balancing = compile_balancing(spec, &build.model.balancing)?;
    Ok(())
}

fn compile_summary_section(build: &mut Build<'_>) -> Result<(), SpecError> {
    let spec = &build.spec;
    build.spec.summaries = compile_summaries(spec, &build.model.summaries)?;
    Ok(())
}

fn compile_default_table_section(build: &mut Build<'_>) -> Result<(), SpecError> {
    build.spec.default_table =
        compile_default_table(&build.spec, build.model.default_table.clone())?;
    Ok(())
}

/// Why a spec's `version` was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum VersionError {
    /// `element` is 0.
    ZeroElement,
    /// `values` is an empty list.
    NoValues,
    /// `values` is left out and the declared element lists no codes.
    NoCodes {
        /// The segment the version declares.
        segment: String,
        /// The 1-based element the version declares.
        element: usize,
    },
    /// A value is the empty string.
    EmptyValue {
        /// The value's 0-based index in `values` as written.
        index: usize,
    },
    /// A value is listed more than once.
    DuplicateValue {
        /// The value as written.
        value: String,
        /// 0-based index of its first listing.
        first: usize,
        /// 0-based index of its repeat.
        second: usize,
    },
}

impl fmt::Display for VersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionError::ZeroElement => write!(f, "\"element\" is 0; positions are 1-based"),
            VersionError::NoValues => write!(
                f,
                "\"values\" is empty; list at least one value the element may hold"
            ),
            VersionError::NoCodes { segment, element } => write!(
                f,
                "\"values\" is absent and element {element} of segment {segment:?} is not defined or lists no codes to take them from; write \"values\", or give segments.{segment}.elements.{element} its \"codes\""
            ),
            VersionError::EmptyValue { index } => write!(f, "\"values[{index}]\" is empty"),
            VersionError::DuplicateValue {
                value,
                first,
                second,
            } => write!(
                f,
                "value {value:?} is listed twice, at values[{first}] and values[{second}]"
            ),
        }
    }
}

/// The values a version lists: those written, else the codes of the declared
/// element, in the order written.
fn values_source<'a>(
    raw: &'a VersionModel,
    segments: &'a BTreeMap<String, SegmentModel>,
) -> Result<&'a [String], VersionError> {
    let codes = segments
        .get(&raw.segment)
        .and_then(|segment| segment.elements.get(&raw.element))
        .and_then(|element| element.codes.as_deref());
    raw.values
        .as_deref()
        .or(codes)
        .ok_or_else(|| VersionError::NoCodes {
            segment: raw.segment.clone(),
            element: raw.element,
        })
}

/// The values a version covers, checked: not empty, none empty, none twice.
fn version_values<'a>(
    raw: &'a VersionModel,
    segments: &'a BTreeMap<String, SegmentModel>,
) -> Result<&'a [String], VersionError> {
    let values = values_source(raw, segments)?;
    if values.is_empty() {
        return Err(VersionError::NoValues);
    }
    if let Some(index) = values.iter().position(String::is_empty) {
        return Err(VersionError::EmptyValue { index });
    }
    let mut seen = std::collections::HashMap::with_capacity(values.len());
    for (second, value) in values.iter().enumerate() {
        if let Some(first) = seen.insert(value.as_str(), second) {
            return Err(VersionError::DuplicateValue {
                value: value.clone(),
                first,
                second,
            });
        }
    }
    Ok(values)
}

/// Validates a spec's `version`. Values left out are the codes of the declared
/// element, in the order written.
fn compile_version(
    raw: &VersionModel,
    segments: &BTreeMap<String, SegmentModel>,
) -> Result<DeclaredVersion, SpecError> {
    if raw.segment.is_empty() {
        return Err(SpecError::EmptySegmentId {
            loop_name: None,
            key: "version.segment".into(),
        });
    }
    let bad = |reason| SpecError::BadVersion { reason };
    if raw.element == 0 {
        return Err(bad(VersionError::ZeroElement));
    }
    let values = version_values(raw, segments).map_err(bad)?;
    Ok(DeclaredVersion {
        segment: raw.segment.as_bytes().to_vec(),
        element: raw.element,
        values: values.iter().map(|v| v.as_bytes().to_vec()).collect(),
    })
}
