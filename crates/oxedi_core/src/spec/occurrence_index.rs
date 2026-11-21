//! The occurrence a segment takes in a loop, found without scanning the loop.

use std::collections::BTreeMap;

use super::Spec;
use super::loops::{LoopDef, LoopId};
use crate::segment::Segment;

/// Position of an occurrence in [`LoopDef::occurrences`]: 0-based. An
/// `OccurrenceId` is only meaningful with the loop it was found in, in the
/// [`Spec`] that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OccurrenceId(usize);

impl OccurrenceId {
    /// 0-based index in the loop's [`LoopDef::occurrences`].
    pub fn index(self) -> usize {
        self.0
    }
}

/// Per loop, the occurrences of each segment id, in spec order: a segment can
/// only take one of those, so only their qualifiers are left to check.
#[derive(Debug, Clone, Default)]
pub(super) struct OccurrenceIndex {
    by_loop: Vec<BTreeMap<Vec<u8>, Vec<OccurrenceId>>>,
}

impl OccurrenceIndex {
    /// The index of `loops`, built once when a spec is assembled.
    pub(super) fn build(loops: &[LoopDef]) -> Self {
        let by_loop = loops
            .iter()
            .map(|def| {
                let mut ids: BTreeMap<Vec<u8>, Vec<OccurrenceId>> = BTreeMap::new();
                for (at, occurrence) in def.occurrences.iter().enumerate() {
                    ids.entry(occurrence.segment.clone())
                        .or_default()
                        .push(OccurrenceId(at));
                }
                ids
            })
            .collect();
        OccurrenceIndex { by_loop }
    }

    fn candidates(&self, loop_id: LoopId, segment_id: &[u8]) -> &[OccurrenceId] {
        self.by_loop
            .get(loop_id.index())
            .and_then(|ids| ids.get(segment_id))
            .map_or(&[], Vec::as_slice)
    }
}

impl Spec {
    /// The occurrence `segment` takes in `loop_id`: the first one with the
    /// segment's id whose qualifier, when it has one, holds. `None` when no
    /// occurrence matches. Qualifiers of one segment never share a code, so
    /// at most one occurrence matches.
    pub fn occurrence_of(&self, loop_id: LoopId, segment: &Segment<'_>) -> Option<OccurrenceId> {
        let occurrences = &self.get(loop_id).occurrences;
        self.occurrence_index
            .candidates(loop_id, segment.id)
            .iter()
            .copied()
            .find(|id| {
                occurrences[id.0]
                    .qualifier
                    .as_ref()
                    .is_none_or(|qualifier| qualifier.matches(segment))
            })
    }
}
