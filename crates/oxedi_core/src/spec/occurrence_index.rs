//! The occurrence a segment takes in a loop, found without scanning the loop.

use super::Spec;
use super::id_map::pack_id;
use super::loops::{LoopDef, LoopId};
use crate::segment::Segment;
use std::collections::BTreeMap;

/// Position of an occurrence in [`LoopDef::occurrences`]: 0-based. An
/// `OccurrenceId` is only meaningful with the loop it was found in, in the
/// [`Spec`] that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OccurrenceId(usize);

impl OccurrenceId {
    /// The occurrence at 0-based `index` of a loop's occurrences.
    pub(super) fn at(index: usize) -> Self {
        OccurrenceId(index)
    }

    /// 0-based index in the loop's [`LoopDef::occurrences`].
    pub fn index(self) -> usize {
        self.0
    }
}

/// The occurrences of one segment id in one loop, in spec order: a segment
/// can only take one of those, so only their qualifiers are left to check.
#[derive(Debug, Clone)]
struct Entry<K> {
    loop_at: usize,
    id: K,
    occurrences: Vec<OccurrenceId>,
}

/// The occurrences of every `(loop, segment id)` pair, in two flat sorted
/// vectors: ids that [`pack_id`] packs by their packed key, and the rare
/// longer ids by their bytes. The short entries of a loop are contiguous and
/// sorted by key, and `loop_starts` says where each loop's run begins, so a
/// lookup binary-searches only that loop's ids in one contiguous array, with
/// no allocation.
#[derive(Debug, Clone, Default)]
pub(crate) struct OccurrenceIndex {
    /// Per loop (by [`LoopId::index`]), where its run of `short` begins; one
    /// more entry ends the last run.
    loop_starts: Vec<usize>,
    short: Vec<Entry<u64>>,
    long: Vec<Entry<Vec<u8>>>,
}

/// Groups the occurrences of `loops` by `(loop, id)`, sorted by that pair.
fn grouped<K: Ord>(loops: &[LoopDef], key: impl Fn(&[u8]) -> Option<K>) -> Vec<Entry<K>> {
    let mut groups: BTreeMap<(usize, K), Vec<OccurrenceId>> = BTreeMap::new();
    for (loop_at, def) in loops.iter().enumerate() {
        for (position, occurrence) in def.occurrences.iter().enumerate() {
            if let Some(id) = key(&occurrence.segment) {
                groups
                    .entry((loop_at, id))
                    .or_default()
                    .push(OccurrenceId(position));
            }
        }
    }
    groups
        .into_iter()
        .map(|((loop_at, id), occurrences)| Entry {
            loop_at,
            id,
            occurrences,
        })
        .collect()
}

impl OccurrenceIndex {
    /// The index of `loops`, built once when a spec is assembled.
    pub(crate) fn build(loops: &[LoopDef]) -> Self {
        let short = grouped(loops, pack_id);
        let loop_starts = (0..=loops.len())
            .map(|loop_at| short.partition_point(|entry| entry.loop_at < loop_at))
            .collect();
        OccurrenceIndex {
            loop_starts,
            short,
            long: grouped(loops, |id| pack_id(id).is_none().then(|| id.to_vec())),
        }
    }

    /// The short entries of the loop at `loop_at`, sorted by packed id.
    fn short_run(&self, loop_at: usize) -> Option<&[Entry<u64>]> {
        let range = *self.loop_starts.get(loop_at)?..*self.loop_starts.get(loop_at + 1)?;
        self.short.get(range)
    }

    fn candidates(&self, loop_id: LoopId, segment_id: &[u8]) -> &[OccurrenceId] {
        let loop_at = loop_id.index();
        let found = match pack_id(segment_id) {
            Some(key) => self.short_run(loop_at).and_then(|run| {
                run.binary_search_by_key(&key, |entry| entry.id)
                    .ok()
                    .map(|at| &run[at].occurrences)
            }),
            None => self
                .long
                .binary_search_by(|entry| {
                    (entry.loop_at, entry.id.as_slice()).cmp(&(loop_at, segment_id))
                })
                .ok()
                .map(|at| &self.long[at].occurrences),
        };
        found.map_or(&[], Vec::as_slice)
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
            .find(|id| occurrences[id.0].qualifier_holds(segment))
    }
}
