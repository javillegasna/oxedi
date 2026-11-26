//! The state of the rows being collected: the slots of a row, the per-table
//! state, and the opening and closing of a row when an anchor loop opens and
//! closes.

use crate::column::Table;
use crate::spec::LoopId;
use crate::spec::{Relation, Watcher};
use crate::vocabulary::Pick;

use super::Projector;
use super::carry::Carried;
use super::fill::append;

/// A column value of a row being collected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Slot {
    /// No segment has matched the column's source yet.
    Unset,
    Null,
    /// A range of the row's byte buffer.
    Bytes(usize, usize),
    Int(i64),
    Decimal(i128),
    Date(i32),
    Time(i32),
}

/// A row being collected.
#[derive(Debug, Clone, Default)]
pub(super) struct Row {
    pub(super) ordinal: usize,
    pub(super) segment: usize,
    pub(super) parents: Vec<Option<usize>>,
    pub(super) cells: Vec<Slot>,
    /// Per column: how many segments have matched its source, for a column
    /// that picks the n-th.
    pub(super) seen: Vec<usize>,
    pub(super) bytes: Vec<u8>,
}

/// What the projector keeps for one table.
#[derive(Debug, Clone)]
pub(super) struct TableState {
    /// Number of the next row.
    pub(super) next: usize,
    /// `true` while an instance of an anchor loop is open (tables without
    /// `segment` only).
    pub(super) open: bool,
    pub(super) row: Row,
    /// The values of the columns that read a loop above the anchor, kept
    /// from the enclosing instance of that loop until a row opens.
    pub(super) carried: Vec<Carried>,
    /// `false` for a table left out of the projection: its rows are still
    /// numbered, so the tables below it keep their parent references, but
    /// no column is read and no row is stored.
    pub(super) kept: bool,
    /// Whether the open row's columns are read: a kept table, or one left
    /// out whose seed column the rows below it shift their dates by.
    pub(super) collect: bool,
    /// Whether the table has columns to mask before a row is appended.
    pub(super) masked: bool,
    pub(super) table: Table,
}

/// Where a column's next value goes: the slot, the count of matches so far
/// and the bytes a text slot ranges over.
#[derive(Debug)]
pub(super) struct Destination<'a> {
    pub(super) slot: &'a mut Slot,
    seen: &'a mut usize,
    pub(super) bytes: &'a mut Vec<u8>,
    /// Whether the value is carried: a new value replaces its bytes.
    carried: bool,
}

impl<'a> Destination<'a> {
    fn carried(carried: &'a mut Carried) -> Self {
        Self {
            slot: &mut carried.slot,
            seen: &mut carried.seen,
            bytes: &mut carried.bytes,
            carried: true,
        }
    }

    /// The destination in the open row `row`, when one is open.
    fn of_row(row: &'a mut Row, column: usize, open: bool) -> Option<Self> {
        open.then_some(())?;
        Some(Self {
            slot: row.cells.get_mut(column)?,
            seen: row.seen.get_mut(column)?,
            bytes: &mut row.bytes,
            carried: false,
        })
    }

    /// The destination, unless `pick` takes the first match and it has come.
    fn unless_picked(self, pick: Pick) -> Option<Self> {
        (pick != Pick::First || *self.slot == Slot::Unset).then_some(self)
    }

    /// Whether the segment that matched is the one `pick` takes. An n-th pick
    /// counts the match; a taken carried value starts over in its bytes.
    pub(super) fn takes(&mut self, pick: Pick) -> bool {
        if let Pick::Nth(nth) = pick {
            *self.seen = self.seen.saturating_add(1);
            if *self.seen != nth {
                return false;
            }
        }
        if self.carried {
            self.bytes.clear();
        }
        true
    }
}

impl TableState {
    /// Where `watcher`'s column takes its value in this table: `None` when
    /// the table does not collect, when the column reads into the open row
    /// and none is open, or when the first match is picked and has come.
    pub(super) fn destination(&mut self, watcher: &Watcher) -> Option<Destination<'_>> {
        if !self.collect {
            return None;
        }
        let destination = match watcher.relation {
            Relation::Above { carried } => Destination::carried(self.carried.get_mut(carried)?),
            Relation::Anchor | Relation::Inside => {
                Destination::of_row(&mut self.row, watcher.column, self.open)?
            }
        };
        destination.unless_picked(watcher.pick)
    }
}

impl Projector<'_> {
    pub(super) fn opened(&mut self, id: LoopId, trigger: usize) {
        self.reset_carried(id);
        let Some(index) = self.spec.reads().anchored(id) else {
            return;
        };
        let state = &mut self.tables[index];
        if !state.collect {
            // Only the number is needed, for the rows below that refer to it.
            state.row.ordinal = state.next;
            state.next = state.next.saturating_add(1);
            state.open = true;
            return;
        }
        let spec = self.spec;
        let def = &spec.tables()[index];
        let mut row = std::mem::take(&mut self.tables[index].row);
        row.parents.clear();
        row.parents
            .extend(def.ancestors.iter().map(|&above| self.open_row(above)));
        row.cells.clear();
        row.cells.resize(def.columns.len(), Slot::Unset);
        row.seen.clear();
        row.seen.resize(def.columns.len(), 0);
        row.bytes.clear();
        row.segment = trigger;
        let state = &mut self.tables[index];
        for carried in &state.carried {
            carried.copy_into(&mut row);
        }
        row.ordinal = state.next;
        state.next = state.next.saturating_add(1);
        state.row = row;
        state.open = true;
    }

    pub(super) fn closed(&mut self, id: LoopId) {
        if let Some(index) = self.spec.reads().anchored(id) {
            let state = &mut self.tables[index];
            let append_row = state.open && state.kept;
            state.open = false;
            if append_row {
                if state.masked {
                    let mut row = std::mem::take(&mut state.row);
                    self.mask_row(index, &mut row);
                    self.tables[index].row = row;
                }
                let state = &mut self.tables[index];
                let dropped = append(&mut state.table, &state.row, &mut self.cells);
                let segment = state.row.segment;
                self.report_dropped(dropped, index, segment);
            }
        }
    }

    /// The number of the row `table` is collecting, if an instance is open.
    pub(super) fn open_row(&self, table: usize) -> Option<usize> {
        let state = &self.tables[table];
        state.open.then_some(state.row.ordinal)
    }
}
