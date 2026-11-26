//! Values read from a loop above a table's anchor: picked while an instance
//! of that loop captures segments, and copied into each row that opens
//! inside it.

use super::Projector;
use super::row::{Row, Slot};
use crate::spec::{ColumnPlace, LoopId};

/// The value a column that reads a loop above its table's anchor holds,
/// with the bytes of a text value and the count of matches so far.
#[derive(Debug, Clone)]
pub(super) struct Carried {
    /// The column, as an index into the table's declared columns.
    pub(super) column: usize,
    pub(super) slot: Slot,
    /// How many segments have matched the column's source in the current
    /// instance, for a column that picks the n-th.
    pub(super) seen: usize,
    /// The bytes a text slot ranges over; a new value replaces them.
    pub(super) bytes: Vec<u8>,
}

impl Carried {
    pub(super) fn new(column: usize) -> Self {
        Self {
            column,
            slot: Slot::Unset,
            seen: 0,
            bytes: Vec::new(),
        }
    }

    /// Forgets the value, as when a new instance of the loop opens.
    pub(super) fn clear(&mut self) {
        self.slot = Slot::Unset;
        self.seen = 0;
        self.bytes.clear();
    }

    /// Writes the value into its column of `row`, copying text into the
    /// row's own bytes.
    pub(super) fn copy_into(&self, row: &mut Row) {
        let slot = match self.slot {
            Slot::Bytes(start, end) => {
                let from = row.bytes.len();
                row.bytes
                    .extend_from_slice(self.bytes.get(start..end).unwrap_or_default());
                Slot::Bytes(from, row.bytes.len())
            }
            other => other,
        };
        if let Some(cell) = row.cells.get_mut(self.column) {
            *cell = slot;
        }
    }
}

/// The carried values of a table, one per column that reads a loop above its
/// anchor, in the order of the carried slots the spec assigned.
pub(super) fn carried(places: &[ColumnPlace]) -> Vec<Carried> {
    let slot_of = |place: &ColumnPlace| place.reading.above().map(|(slot, _)| slot);
    let count = places.iter().filter_map(slot_of).count();
    (0..count)
        .filter_map(|slot| places.iter().position(|place| slot_of(place) == Some(slot)))
        .map(Carried::new)
        .collect()
}

impl Projector<'_> {
    /// Forgets the values read from loop `id`: an instance of it opens.
    pub(super) fn reset_carried(&mut self, id: LoopId) {
        for &(table, at) in self.spec.reads().resets(id) {
            if let Some(carried) = self
                .tables
                .get_mut(table)
                .and_then(|state| state.carried.get_mut(at))
            {
                carried.clear();
            }
        }
    }
}
