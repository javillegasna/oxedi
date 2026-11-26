//! The compilation of one loop: the checks of its own keys, its parent, its
//! trigger and its control, why a control is rejected, the parent cycle check
//! over every loop, and the segments its occurrences hold.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use super::error::SpecError;
use super::model::{ControlModel, LoopModel};
use crate::delimiters::HeaderPositions;
use crate::spec::{Control, EnvelopeField, EnvelopeFields, LoopDef, LoopId, Spec, Trigger};

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
    /// A field sits on the element that holds the control number.
    FieldOnOpener {
        /// The 1-based element, the opener's.
        element: usize,
        /// The field the spec puts there.
        field: EnvelopeField,
    },
    /// A field sits on a place of the interchange header the syntax layer owns.
    FieldOnHeaderPlace {
        /// The 1-based element, the place's.
        element: usize,
        /// What the place holds, e.g. `repetition separator`.
        place: &'static str,
        /// The field the spec puts there.
        field: EnvelopeField,
        /// The trigger segment.
        segment: String,
    },
    /// A field sits on an element the trigger segment does not define.
    FieldNotOnTrigger {
        /// The 1-based element the spec puts the field on.
        element: usize,
        /// The field the spec puts there.
        field: EnvelopeField,
        /// The trigger segment.
        segment: String,
    },
    /// One field sits on two elements.
    FieldTwice {
        /// The field written twice.
        field: EnvelopeField,
        /// The lower element.
        first: usize,
        /// The higher element.
        second: usize,
    },
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
        fields: compile_fields(raw)?,
    })
}

// The fields by element, in ascending element order, each on one element and
// none on the one that holds the control number.
fn compile_fields(raw: &ControlModel) -> Result<EnvelopeFields, ControlError> {
    raw.fields
        .iter()
        .try_fold(EnvelopeFields::default(), |placed, (&element, &field)| {
            if element == raw.opener_element {
                return Err(ControlError::FieldOnOpener { element, field });
            }
            placed
                .place(element, field)
                .map_err(|first| ControlError::FieldTwice {
                    field,
                    first,
                    second: element,
                })
        })
}

/// Fails with the first caller field of a loop's `control` that sits on an
/// element its trigger segment does not define, or on a place of the top
/// loop's interchange header the syntax layer owns. Runs after the segments
/// compile, since loops compile before them.
pub(super) fn check_control_fields(spec: &Spec) -> Result<(), SpecError> {
    let fault = spec.loops().iter().find_map(|def| {
        let control = def.control?;
        let reason = control
            .fields
            .by_element()
            .into_iter()
            .find_map(|(element, field)| field_fault(spec, def, element, field))?;
        Some(SpecError::BadControl {
            loop_name: def.name.clone(),
            reason,
        })
    });
    fault.map_or(Ok(()), Err)
}

// The places of the interchange header the syntax layer owns, by element.
const HEADER_PLACES: [(usize, &str); 3] = [
    (HeaderPositions::ISA.repetition, "repetition separator"),
    (HeaderPositions::ISA.version, "interchange version"),
    (HeaderPositions::ISA.component, "component separator"),
];

// What is wrong with `field` on `element` of `def`'s trigger, if anything.
fn field_fault(
    spec: &Spec,
    def: &LoopDef,
    element: usize,
    field: EnvelopeField,
) -> Option<ControlError> {
    let segment = String::from_utf8_lossy(&def.trigger.segment).into_owned();
    let owned = HEADER_PLACES.iter().find(|(at, _)| *at == element);
    if let (None, Some(&(_, place))) = (def.parent, owned) {
        return Some(ControlError::FieldOnHeaderPlace {
            element,
            place,
            field,
            segment,
        });
    }
    spec.element_def(&def.trigger.segment, element, None)
        .is_none()
        .then_some(ControlError::FieldNotOnTrigger {
            element,
            field,
            segment,
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
