//! A map keyed by segment id.

use std::collections::BTreeMap;

/// A map from segment id to `V`: the ids map to slots of `values`, so the
/// map code is shared by every `V`.
#[derive(Debug, Clone)]
pub(crate) struct IdMap<V> {
    slots: IdSlots,
    values: Vec<V>,
}

/// The slot of each id. Ids that [`pack_id`] can pack are keyed by their
/// packed bytes.
#[derive(Debug, Clone, Default)]
struct IdSlots {
    short: BTreeMap<u64, usize>,
    long: BTreeMap<Vec<u8>, usize>,
}

impl IdSlots {
    fn get(&self, id: &[u8]) -> Option<usize> {
        match pack_id(id) {
            Some(key) => self.short.get(&key).copied(),
            None => self.long.get(id).copied(),
        }
    }

    /// The slot of `id`, which is `next` when `id` had none.
    fn slot_or(&mut self, id: &[u8], next: usize) -> usize {
        match pack_id(id) {
            Some(key) => *self.short.entry(key).or_insert(next),
            None => *self.long.entry(id.to_vec()).or_insert(next),
        }
    }
}

impl<V> Default for IdMap<V> {
    fn default() -> Self {
        IdMap {
            slots: IdSlots::default(),
            values: Vec::new(),
        }
    }
}

/// The packed key of an id, or `None` when it is longer than seven bytes. A
/// packed key compares in one word, faster than a slice, and a leading 1 bit
/// keeps ids of different lengths apart.
pub(crate) fn pack_id(id: &[u8]) -> Option<u64> {
    (id.len() <= 7).then(|| {
        id.iter()
            .fold(1u64, |key, &byte| key << 8 | u64::from(byte))
    })
}

impl<V> IdMap<V> {
    pub(crate) fn get(&self, id: &[u8]) -> Option<&V> {
        self.slots.get(id).and_then(|slot| self.values.get(slot))
    }

    /// The value of `id`, made with `make` when there is none.
    pub(crate) fn entry_or_insert_with(&mut self, id: &[u8], make: impl FnOnce() -> V) -> &mut V {
        let next = self.values.len();
        let slot = self.slots.slot_or(id, next);
        if slot == next {
            self.values.push(make());
        }
        // `slot` is `next`, just pushed, or a slot an earlier call pushed.
        &mut self.values[slot]
    }
}
