//! Construction of a [`Spec`] from JSON: loading, control compilation and structural checks.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use super::Spec;
use super::compile::compile_tables;
use super::error::SpecError;
use super::loops::{Control, ControlCount, ControlError, LoopDef, LoopId, Trigger};
use super::occurrences::compile_occurrences;
use super::raw::{RawControl, RawLoop, RawSegment, RawSpec};
use super::render::render_trigger;
use super::segments::{SegmentDef, compile_elements, parse_position};
use super::shape::{check_shape, section};
use super::version::compile_version;

impl Spec {
    pub(crate) fn from_value(source: Value) -> Result<Spec, SpecError> {
        check_shape(&source)?;
        let raw = RawSpec::deserialize(&source).map_err(|e| SpecError::Schema {
            loop_name: None,
            source: e,
        })?;
        if raw.loops.is_empty() {
            return Err(SpecError::NoLoops {
                spec_name: raw.name,
            });
        }
        let mut defs = Vec::with_capacity(raw.loops.len());
        for (name, value) in section(&source, "loops") {
            let def = RawLoop::deserialize(value).map_err(|e| SpecError::Schema {
                loop_name: Some(name.to_string()),
                source: e,
            })?;
            defs.push((name.to_string(), def));
        }
        let names: Vec<&str> = raw.loops.keys().map(String::as_str).collect();
        let id_of = |name: &str| names.iter().position(|&n| n == name).map(LoopId);

        let mut loops = Vec::with_capacity(defs.len());
        for (name, def) in &defs {
            let empty_at = |key: String| SpecError::EmptySegmentId {
                loop_name: Some(name.clone()),
                key,
            };
            if def.trigger.segment.is_empty() {
                return Err(empty_at("trigger.segment".into()));
            }
            if def.end.as_deref() == Some("") {
                return Err(empty_at("end".into()));
            }
            if def.max == Some(0) {
                return Err(SpecError::ZeroLoopMax {
                    loop_name: name.clone(),
                });
            }
            if def.end.as_deref() == Some(def.trigger.segment.as_str()) {
                return Err(SpecError::EndIsTrigger {
                    loop_name: name.clone(),
                    segment: def.trigger.segment.clone(),
                });
            }
            let parent = match &def.parent {
                None => None,
                Some(parent) => Some(id_of(parent).ok_or_else(|| SpecError::UnknownParent {
                    loop_name: name.clone(),
                    parent: parent.clone(),
                })?),
            };
            let mut conditions = Vec::with_capacity(def.trigger.conditions.len());
            for (position, value) in &def.trigger.conditions {
                let parsed = parse_position(position).ok_or_else(|| SpecError::BadPosition {
                    loop_name: name.clone(),
                    position: position.clone(),
                })?;
                conditions.push((parsed, value.as_bytes().to_vec()));
            }
            conditions.sort();
            let control = match &def.control {
                None => None,
                Some(raw) => Some(compile_control(raw, def.end.is_some()).map_err(|reason| {
                    SpecError::BadControl {
                        loop_name: name.clone(),
                        reason,
                    }
                })?),
            };
            loops.push(LoopDef {
                name: name.clone(),
                parent,
                trigger: Trigger {
                    segment: def.trigger.segment.as_bytes().to_vec(),
                    conditions,
                },
                occurrences: Vec::new(),
                max: def.max,
                segments: Vec::new(),
                end: def.end.as_ref().map(|s| s.as_bytes().to_vec()),
                control,
                children: Vec::new(),
            });
        }

        check_cycles(&loops)?;

        let parents: Vec<Option<LoopId>> = loops.iter().map(|def| def.parent).collect();
        let mut roots = Vec::new();
        for (index, parent) in parents.into_iter().enumerate() {
            match parent {
                Some(parent) => loops[parent.0].children.push(LoopId(index)),
                None => roots.push(LoopId(index)),
            }
        }

        let mut segments = BTreeMap::new();
        for (id, value) in section(&source, "segments") {
            if id.is_empty() {
                return Err(SpecError::EmptySegmentId {
                    loop_name: None,
                    key: "segments.\"\"".into(),
                });
            }
            let def = RawSegment::deserialize(value).map_err(|e| SpecError::SegmentSchema {
                segment: id.to_string(),
                source: e,
            })?;
            let elements = compile_elements(id, &def.elements, None)?;
            segments.insert(id.as_bytes().to_vec(), SegmentDef { elements });
        }

        for ((name, def), compiled) in defs.iter().zip(loops.iter_mut()) {
            compiled.occurrences = compile_occurrences(name, &def.occurrences, &segments)?;
            if !compiled.occurrences.is_empty()
                && !compiled
                    .occurrences
                    .iter()
                    .any(|occurrence| occurrence.opens_on(&compiled.trigger))
            {
                return Err(SpecError::UnmatchedTrigger {
                    loop_name: name.clone(),
                    trigger: render_trigger(&compiled.trigger),
                });
            }
            for occurrence in &compiled.occurrences {
                if !occurrence.opens_on(&compiled.trigger)
                    && !compiled.segments.contains(&occurrence.segment)
                {
                    compiled.segments.push(occurrence.segment.clone());
                }
            }
        }

        let version = raw.version.as_ref().map(compile_version).transpose()?;
        let mut spec = Spec {
            name: raw.name,
            loops,
            roots,
            segments,
            tables: Vec::new(),
            version,
            source,
        };
        spec.check_ambiguity()?;
        spec.tables = compile_tables(&spec, section(&spec.source, "tables"))?;
        Ok(spec)
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

/// Validates a loop's `control`; `has_end` says whether the loop declares an end segment.
fn compile_control(raw: &RawControl, has_end: bool) -> Result<Control, ControlError> {
    if !has_end {
        return Err(ControlError::NoEnd);
    }
    let positions = [
        ("opener_element", raw.opener_element),
        ("closer_element", raw.closer_element),
        ("count_element", raw.count_element),
    ];
    if let Some((key, _)) = positions.iter().find(|(_, position)| *position == 0) {
        return Err(ControlError::ZeroPosition { key });
    }
    let count = match raw.count.as_str() {
        "segments" => ControlCount::Segments,
        "children" => ControlCount::Children,
        _ => {
            return Err(ControlError::UnknownCount {
                found: raw.count.clone(),
            });
        }
    };
    Ok(Control {
        opener_element: raw.opener_element,
        closer_element: raw.closer_element,
        count_element: raw.count_element,
        count,
    })
}

/// Fails with the first parent cycle found, walking from each loop in spec
/// order. The members start at the loop the walk reaches twice.
fn check_cycles(loops: &[LoopDef]) -> Result<(), SpecError> {
    for start in 0..loops.len() {
        let mut walk: Vec<LoopId> = Vec::new();
        let mut current = Some(LoopId(start));
        while let Some(id) = current {
            if let Some(repeat) = walk.iter().position(|&seen| seen == id) {
                let members = walk[repeat..]
                    .iter()
                    .map(|&member| loops[member.0].name.clone())
                    .collect();
                return Err(SpecError::Cycle { members });
            }
            walk.push(id);
            current = loops[id.0].parent;
        }
    }
    Ok(())
}
