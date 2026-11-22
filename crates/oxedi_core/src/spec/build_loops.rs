//! The compilation of one loop: the checks of its own keys, its parent, its
//! trigger and its control, why a control is rejected, the parent cycle check
//! over every loop, and the segments its occurrences hold.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use super::error::SpecError;
use super::model::{ControlModel, LoopModel};
use crate::spec::{Control, LoopDef, LoopId, Trigger};

/// A check of a loop's own keys: the error when the loop breaks it.
type HeaderRule = fn(&str, &LoopModel) -> Option<SpecError>;

/// The checks of a loop's keys that need nothing but the loop, in the order they run.
const HEADER_RULES: [HeaderRule; 4] = [empty_trigger, empty_end, zero_max, end_is_trigger];

fn empty_trigger(name: &str, def: &LoopModel) -> Option<SpecError> {
    def.trigger
        .segment
        .is_empty()
        .then(|| empty_segment_id(name, "trigger.segment"))
}

fn empty_end(name: &str, def: &LoopModel) -> Option<SpecError> {
    (def.end.as_deref() == Some("")).then(|| empty_segment_id(name, "end"))
}

fn empty_segment_id(name: &str, key: &str) -> SpecError {
    SpecError::EmptySegmentId {
        loop_name: Some(name.to_string()),
        key: key.to_string(),
    }
}

fn zero_max(name: &str, def: &LoopModel) -> Option<SpecError> {
    (def.max == Some(0)).then(|| SpecError::ZeroLoopMax {
        loop_name: name.to_string(),
    })
}

fn end_is_trigger(name: &str, def: &LoopModel) -> Option<SpecError> {
    (def.end.as_deref() == Some(def.trigger.segment.as_str())).then(|| SpecError::EndIsTrigger {
        loop_name: name.to_string(),
        segment: def.trigger.segment.clone(),
    })
}

/// A loop as written, without its occurrences and children, which later steps add.
pub(super) fn compile_loop(
    name: &str,
    def: &LoopModel,
    ids: &BTreeMap<&str, LoopId>,
) -> Result<LoopDef, SpecError> {
    if let Some(error) = HEADER_RULES.iter().find_map(|rule| rule(name, def)) {
        return Err(error);
    }
    Ok(LoopDef {
        name: name.to_string(),
        parent: parent_id(name, def, ids)?,
        trigger: Trigger {
            segment: def.trigger.segment.as_bytes().to_vec(),
            conditions: def
                .trigger
                .conditions
                .iter()
                .map(|(&position, value)| (position, value.as_bytes().to_vec()))
                .collect(),
        },
        occurrences: Vec::new(),
        usage: def.usage,
        max: def.max,
        segments: Vec::new(),
        end: def.end.as_ref().map(|s| s.as_bytes().to_vec()),
        control: loop_control(name, def)?,
        children: Vec::new(),
    })
}

fn parent_id(
    name: &str,
    def: &LoopModel,
    ids: &BTreeMap<&str, LoopId>,
) -> Result<Option<LoopId>, SpecError> {
    let Some(parent) = &def.parent else {
        return Ok(None);
    };
    match ids.get(parent.as_str()) {
        Some(&id) => Ok(Some(id)),
        None => Err(SpecError::UnknownParent {
            loop_name: name.to_string(),
            parent: parent.clone(),
        }),
    }
}

fn loop_control(name: &str, def: &LoopModel) -> Result<Option<Control>, SpecError> {
    def.control
        .as_ref()
        .map(|raw| {
            compile_control(raw, def.end.is_some()).map_err(|reason| SpecError::BadControl {
                loop_name: name.to_string(),
                reason,
            })
        })
        .transpose()
}

/// The segment ids a loop's occurrences hold, other than the one it opens
/// on, each once, in occurrence order.
pub(super) fn held_segments(def: &LoopDef) -> Vec<Vec<u8>> {
    let mut seen = BTreeSet::new();
    def.occurrences
        .iter()
        .filter(|occurrence| !occurrence.opens_on(&def.trigger))
        .filter(|occurrence| seen.insert(occurrence.segment.as_slice()))
        .map(|occurrence| occurrence.segment.clone())
        .collect()
}

/// Why a loop's `control` was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlError {
    /// A position key holds 0.
    ZeroPosition {
        /// The key, e.g. `opener_element`.
        key: &'static str,
    },
    /// The loop has no `end` segment to carry the count and control number.
    NoEnd,
}

impl fmt::Display for ControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ControlError::ZeroPosition { key } => {
                write!(f, "{key:?} must be a 1-based element position; found 0")
            }
            ControlError::NoEnd => write!(f, "the loop has no \"end\" segment to check"),
        }
    }
}

/// Validates a loop's `control`; `has_end` says whether the loop declares an end segment.
fn compile_control(raw: &ControlModel, has_end: bool) -> Result<Control, ControlError> {
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
    Ok(Control {
        opener_element: raw.opener_element,
        closer_element: raw.closer_element,
        count_element: raw.count_element,
        count: raw.count,
    })
}

/// Fails with the first parent cycle found, walking from each loop in spec
/// order. The members start at the loop the walk reaches twice.
pub(super) fn check_cycles(loops: &[LoopDef]) -> Result<(), SpecError> {
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
