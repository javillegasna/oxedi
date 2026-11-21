//! A map keyed by segment id.

use std::collections::BTreeMap;

/// A map from segment id to `V`. Ids of up to seven bytes are keyed by their
/// packed bytes, which compare faster than slices.
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

impl<V> IdMap<V> {
    /// The packed key of an id, or `None` when it is too long to pack. A
    /// leading 1 bit keeps ids of different lengths apart.
    fn key(id: &[u8]) -> Option<u64> {
        (id.len() <= 7).then(|| {
            id.iter()
                .fold(1u64, |key, &byte| key << 8 | u64::from(byte))
        })
    }

    pub(crate) fn get(&self, id: &[u8]) -> Option<&V> {
        match Self::key(id) {
            Some(key) => self.short.get(&key),
            None => self.long.get(id),
        }
    }

    pub(crate) fn get_mut(&mut self, id: &[u8]) -> Option<&mut V> {
        match Self::key(id) {
            Some(key) => self.short.get_mut(&key),
            None => self.long.get_mut(id),
        }
    }

    /// The value of `id`, made with `make` when there is none.
    pub(crate) fn entry_or_insert_with(&mut self, id: &[u8], make: impl FnOnce() -> V) -> &mut V {
        match Self::key(id) {
            Some(key) => self.short.entry(key).or_insert_with(make),
            None => self.long.entry(id.to_vec()).or_insert_with(make),
        }
    }
}
