//! Loop identifiers, triggers, loop definitions and control declarations.

#[cfg(doc)]
use super::Spec;
use super::occurrences::{OccurrenceDef, Usage};
use crate::segment::Segment;

/// Index of a loop definition inside a [`Spec`]. A `LoopId` is only
/// meaningful for the `Spec` that produced it: using it with another spec,
/// including one produced by [`LoadedSpec::merge_patch`](super::LoadedSpec::merge_patch), indexes a different loop
/// or panics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoopId(pub(super) usize);

impl LoopId {
    /// Position of the loop in [`Spec::loops`].
    pub fn index(self) -> usize {
        self.0
    }
}

/// What opens a loop: a segment id plus optional element conditions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    /// Segment id, e.g. `CLP`.
    pub segment: Vec<u8>,
    /// `(1-based element position, required value)`, sorted by position.
    /// A condition holds only on a simple element: a composite never
    /// satisfies it, unlike an occurrence's qualifier, which reads a
    /// composite named without a component at its first component.
    pub conditions: Vec<(usize, Vec<u8>)>,
}

impl Trigger {
    /// `true` when the segment has this id and every condition holds.
    pub fn matches(&self, segment: &Segment<'_>) -> bool {
        segment.id == self.segment.as_slice() && segment.holds(&self.conditions)
    }
}

/// One loop of the structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopDef {
    /// Name used in the JSON, e.g. `2100`.
    pub name: String,
    /// `None` for a top-level loop.
    pub parent: Option<LoopId>,
    /// What opens the loop. The trigger segment is captured by the loop.
    pub trigger: Trigger,
    /// The named places segments take in the loop, by position, the one its
    /// trigger opens on included. Empty when the loop declares none: it then
    /// holds only its trigger and its end.
    pub occurrences: Vec<OccurrenceDef>,
    /// Whether every instance of the parent holds an instance of the loop.
    pub usage: Usage,
    /// Most instances the loop may have under one parent instance; `None`
    /// for no limit.
    pub max: Option<usize>,
    /// Segment ids the loop holds after its trigger, derived from the
    /// occurrences: each id once, in position order, leaving out the
    /// occurrences the trigger opens on.
    pub segments: Vec<Vec<u8>>,
    /// Segment that is captured and then closes the loop, e.g. `SE`.
    pub end: Option<Vec<u8>>,
    /// How the end segment checks the loop it closes, for envelope loops.
    pub control: Option<Control>,
    /// Loops whose parent is this one, in spec order.
    pub children: Vec<LoopId>,
}

/// What a loop's end segment counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCount {
    /// Every segment from the trigger to the end segment, both included.
    Segments,
    /// The child loop instances opened by their own trigger.
    Children,
}

/// The control elements an envelope loop's trigger and end segment carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Control {
    /// 1-based position of the control number in the trigger, e.g. `ST02`.
    pub opener_element: usize,
    /// 1-based position of the same control number in the end segment, e.g. `SE02`.
    pub closer_element: usize,
    /// 1-based position of the count in the end segment, e.g. `SE01`.
    pub count_element: usize,
    /// What the count counts.
    pub count: ControlCount,
}

impl LoopDef {
    /// Index of the occurrence the loop's trigger opens on, if any.
    pub(crate) fn trigger_occurrence(&self) -> Option<usize> {
        self.occurrences
            .iter()
            .position(|occurrence| occurrence.opens_on(&self.trigger))
    }

    /// `true` when the loop holds `id` (as a listed segment or as its end).
    pub fn accepts(&self, id: &[u8]) -> bool {
        self.segments.iter().any(|segment| segment == id) || self.end.as_deref() == Some(id)
    }
}
