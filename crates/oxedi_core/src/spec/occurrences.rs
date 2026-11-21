//! Segment occurrences: the named places a segment takes inside a loop, with
//! their usage, maximum repeat, position, qualifier and own code lists.

use std::collections::BTreeMap;

use super::elements::{ElementDef, SegmentDef, holds_code};
use super::loops::Trigger;
use crate::segment::Segment;

/// Whether an occurrence must appear in every instance of its loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Usage {
    /// Written `required`: every instance of the loop holds it.
    Required,
    /// Written `situational`: an instance may hold it or not; the default.
    #[default]
    Situational,
}

/// The element (or component) and the codes that tell an occurrence apart
/// from the other occurrences of its segment in the same loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Qualifier {
    /// 1-based element position.
    pub element: usize,
    /// 1-based component position, when the qualifier is a component.
    pub component: Option<usize>,
    /// The codes that identify the occurrence, sorted by their bytes.
    pub codes: Vec<String>,
}

impl Qualifier {
    /// `true` when the segment holds one of the codes at the qualifier's place.
    /// A simple element counts as its own first component, and a composite
    /// element named without a component is read at its first component.
    #[inline]
    pub fn matches(&self, segment: &Segment<'_>) -> bool {
        segment
            .leaf(self.element, self.component)
            .is_some_and(|value| holds_code(&self.codes, value))
    }
}

/// One named place a segment takes inside a loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OccurrenceDef {
    /// Name used in the JSON, unique within the loop, e.g. `patient_name`.
    pub name: String,
    /// Segment id, e.g. `NM1`.
    pub segment: Vec<u8>,
    /// Position in the loop; occurrences sharing a position may appear in
    /// any order among themselves, and the occurrence the loop's trigger
    /// opens on has the lowest position of its loop. A child loop whose
    /// trigger occurrence has a higher position than its parent's shares the
    /// parent's position space, so its instances are ordered among the
    /// parent's occurrences (in the built-in specs, the transaction and every
    /// loop below it). A child whose trigger occurrence does not come after
    /// its parent's numbers its own positions (the envelope loops above the
    /// transaction).
    pub pos: usize,
    /// Whether every instance of the loop holds it.
    pub usage: Usage,
    /// Most times it may repeat in one instance of the loop; `None` for no limit.
    pub max: Option<usize>,
    /// What tells it apart from the other occurrences of its segment.
    pub qualifier: Option<Qualifier>,
    /// Code lists of its own, by `(element, component)`, each sorted by bytes.
    pub codes: BTreeMap<(usize, Option<usize>), Vec<String>>,
}

impl OccurrenceDef {
    /// `true` when the segment has this occurrence's id and, when it has a
    /// qualifier, one of its codes.
    pub fn matches(&self, segment: &Segment<'_>) -> bool {
        segment.id == self.segment.as_slice() && self.qualifier_holds(segment)
    }

    /// `true` when the occurrence has no qualifier, or the segment holds one
    /// of its codes. The segment's id is not compared.
    pub(super) fn qualifier_holds(&self, segment: &Segment<'_>) -> bool {
        self.qualifier
            .as_ref()
            .is_none_or(|qualifier| qualifier.matches(segment))
    }

    /// `true` when the trigger opens the loop on this occurrence: same
    /// segment and, when the occurrence has a qualifier on an element, a
    /// trigger condition on that element with one of its codes.
    pub(super) fn opens_on(&self, trigger: &Trigger) -> bool {
        if self.segment != trigger.segment {
            return false;
        }
        let Some(qualifier) = &self.qualifier else {
            return true;
        };
        qualifier.component.is_none()
            && trigger.conditions.iter().any(|(position, value)| {
                *position == qualifier.element && holds_code(&qualifier.codes, value)
            })
    }
}

/// The definition of an element or component of `segment`, if any. It takes
/// the `segments` section rather than a [`Spec`](super::Spec) because it also
/// runs while the spec is being built; [`Spec::element_def`](super::Spec::element_def) calls it.
pub(super) fn element_def<'d>(
    segments: &'d BTreeMap<Vec<u8>, SegmentDef>,
    segment: &[u8],
    element: usize,
    component: Option<usize>,
) -> Option<&'d ElementDef> {
    let def = segments.get(segment)?.elements.get(&element)?;
    match component {
        None => Some(def),
        Some(component) => def.composite.get(&component),
    }
}
