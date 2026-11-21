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

/// Per segment id, then per loop, the occurrences of the id in the loop, in
/// spec order: a segment can only take one of those, so only their
/// qualifiers are left to check. Ids of up to seven bytes are keyed by their
/// packed bytes, which compare faster than slices.
#[derive(Debug, Clone, Default)]
pub(super) struct OccurrenceIndex {
    short: BTreeMap<u64, Vec<Vec<OccurrenceId>>>,
    long: BTreeMap<Vec<u8>, Vec<Vec<OccurrenceId>>>,
}

/// The packed key of a segment id, or `None` when it is too long to pack. A
/// leading 1 bit keeps ids of different lengths apart.
pub(crate) fn pack_id(id: &[u8]) -> Option<u64> {
    (id.len() <= 7).then(|| {
        id.iter()
            .fold(1u64, |key, &byte| key << 8 | u64::from(byte))
    })
}

impl OccurrenceIndex {
    /// The index of `loops`, built once when a spec is assembled.
    pub(super) fn build(loops: &[LoopDef]) -> Self {
        let mut index = OccurrenceIndex::default();
        for (at, def) in loops.iter().enumerate() {
            for (position, occurrence) in def.occurrences.iter().enumerate() {
                let per_loop = match pack_id(&occurrence.segment) {
                    Some(key) => index.short.entry(key).or_default(),
                    None => index.long.entry(occurrence.segment.clone()).or_default(),
                };
                per_loop.resize_with(loops.len(), Vec::new);
                per_loop[at].push(OccurrenceId(position));
            }
        }
        index
    }

    fn candidates(&self, loop_id: LoopId, segment_id: &[u8]) -> &[OccurrenceId] {
        let per_loop = match pack_id(segment_id) {
            Some(key) => self.short.get(&key),
            None => self.long.get(segment_id),
        };
        per_loop
            .and_then(|per_loop| per_loop.get(loop_id.index()))
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
