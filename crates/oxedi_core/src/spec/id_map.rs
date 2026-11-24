//! A map keyed by segment id.

use std::collections::BTreeMap;

/// A map from segment id to `V`. Ids that [`pack_id`] can pack are keyed by
/// their packed bytes.
#[derive(Debug, Clone)]
pub(crate) struct IdMap<V> {
    short: BTreeMap<u64, V>,
    long: BTreeMap<Vec<u8>, V>,
}

impl<V> Default for IdMap<V> {
    fn default() -> Self {
        IdMap {
            short: BTreeMap::new(),
            long: BTreeMap::new(),
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
        match pack_id(id) {
            Some(key) => self.short.get(&key),
            None => self.long.get(id),
        }
    }

    pub(crate) fn get_mut(&mut self, id: &[u8]) -> Option<&mut V> {
        match pack_id(id) {
            Some(key) => self.short.get_mut(&key),
            None => self.long.get_mut(id),
        }
    }

    /// The value of `id`, made with `make` when there is none.
    pub(crate) fn entry_or_insert_with(&mut self, id: &[u8], make: impl FnOnce() -> V) -> &mut V {
        match pack_id(id) {
            Some(key) => self.short.entry(key).or_insert_with(make),
            None => self.long.entry(id.to_vec()).or_insert_with(make),
        }
    }

    /// Every value, short ids first, each part in key order.
    #[cfg(test)]
    pub(crate) fn values(&self) -> impl Iterator<Item = &V> {
        self.short.values().chain(self.long.values())
    }
}
