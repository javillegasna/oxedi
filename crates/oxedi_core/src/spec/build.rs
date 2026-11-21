//! Construction of a [`Spec`] from JSON.
//!
//! A spec compiles as a pipeline: `STEPS` runs in order over a `Build`, one
//! step per section or structural check, each reading what the steps before
//! it built.

use std::collections::BTreeMap;

use serde_json::Value;

use super::Spec;
use super::balance::compile_balancing;
use super::build_loops::{check_cycles, compile_loop, held_segments};
use super::compile::compile_tables;
use super::error::SpecError;
use super::loops::{LoopId, Trigger};
use super::model::{SegmentModel, SpecModel};
use super::occurrence_index::OccurrenceIndex;
use super::occurrences::{check_trigger, compile_occurrences};
use super::render::render_trigger;
use super::segments::{SegmentDef, compile_elements};
use super::summaries::{compile_default_table, compile_summaries};
use super::version::compile_version;

/// A spec while it compiles, and the model every step reads.
struct Build<'m> {
    spec: Spec,
    /// The spec as its model reads it, keys in name order.
    model: &'m SpecModel,
}

type Step = fn(&mut Build<'_>) -> Result<(), SpecError>;

/// The compile steps, in the order they run.
const STEPS: [Step; 11] = [
    compile_loop_headers,
    link_loops,
    compile_segment_section,
    compile_loop_occurrences,
    index_occurrences,
    compile_version_section,
    check_trigger_ambiguity,
    compile_table_section,
    compile_balancing_section,
    compile_summary_section,
    compile_default_table_section,
];

impl Spec {
    pub(crate) fn from_value(source: Value) -> Result<Spec, SpecError> {
        let model = SpecModel::from_value(&source)?;
        Spec::build(&model, source)
    }

    /// Compiles `model`; `source` is kept as the spec's JSON for `merge_patch`
    /// and `to_json`.
    pub(in crate::spec) fn build(model: &SpecModel, source: Value) -> Result<Spec, SpecError> {
        let mut build = Build::new(model, source)?;
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
    fn new(model: &'m SpecModel, source: Value) -> Result<Build<'m>, SpecError> {
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
                balancing: Vec::new(),
                summaries: Vec::new(),
                default_table: None,
                version: None,
                source,
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
        .map(compile_version)
        .transpose()?;
    Ok(())
}

fn check_trigger_ambiguity(build: &mut Build<'_>) -> Result<(), SpecError> {
    build.spec.check_ambiguity()
}

fn compile_table_section(build: &mut Build<'_>) -> Result<(), SpecError> {
    let spec = &build.spec;
    build.spec.tables = compile_tables(spec, &build.model.tables)?;
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
