//! Projection of a segment stream into typed tables, with element checks.
//!
//! The projector follows the loops the engine opens and closes and fills the
//! tables of the spec. A table without a `segment` gets one row per instance
//! of its anchor loops: the row number is taken when the instance opens, each
//! column fills from the captured segment its pick chooses among those that
//! match its source (the first by default; see [`crate::spec::Pick`]) while
//! the instance is open, and the row is appended when the instance closes. A
//! column that reads a loop above the anchor keeps the value it picks from
//! the enclosing instance of that loop, which starts over when an instance
//! of the loop opens, and each row copies it when it opens. A
//! table anchored on a segment gets its rows when that segment is captured,
//! one per element group when the segment repeats a group. Every row also
//! carries its number, the index of its anchor segment and, for each table
//! above it, the number of that table's open row (null when none is open).
//! Row numbers count from 0 across the whole stream, so they keep their
//! meaning when the tables are drained part way.
//!
//! Every captured segment the spec defines is checked element by element as
//! it arrives: required elements, types, lengths, codes and composite
//! shapes. An element's codes come from the occurrence the segment takes in
//! its loop when that occurrence lists its own, and from the element's
//! definition otherwise (also for a segment that matches no occurrence). An
//! element defined without components is read as one text, with any
//! component separator it contains kept in place; components are read only
//! where the definition declares them. A column reads the value the check
//! already parsed, so no element is parsed twice. In a text column an
//! element (or component) the segment does not have is null and one it has
//! but leaves empty is the empty text. In the other columns a value that is
//! missing, empty or does not parse as its type is null. A value whose
//! length is out of range is reported and kept.
//!
//! Allocation: appending a row collects its cells into a vector that is kept
//! from one row to the next and grows the table's buffers; each diagnostic
//! owns its text. A row being collected
//! copies the bytes of its text columns into a buffer that is reused from one
//! instance to the next, and the per-segment state (the checked values and
//! the text of a composite read as one) lives in buffers that are cleared and
//! reused.
//!
//! Limit: a text column addresses its bytes with `i32` offsets, so it holds at
//! most `i32::MAX` bytes between two drains of the tables. A text value that
//! would go past it is not stored: its cell is null, the row is kept and a
//! level-2 diagnostic names the table, the column, the row's anchor segment
//! and the byte total. Draining the tables per transaction keeps columns far
//! below the limit.
//!
//! The module is split by responsibility: `plan` holds the per-table plans
//! built from the spec; `check` the element check and the parsed values it
//! hands to the columns; `fill` the row filling, the diagnostic helpers and
//! the text read of an element; `carry` the values read from loops above a
//! table's anchor. This file holds the projector, its event
//! handlers and the row and slot state.

mod carry;
mod check;
mod fill;
mod plan;
#[cfg(test)]
mod source_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod unread_tests;

use crate::column::{Cell, ColumnType, Table, Tables};
use crate::delimiters::Delimiters;
use crate::diagnostic::Diagnostic;
use crate::engine::Event;
use crate::segment::Segment;
use crate::spec::{LoopId, OccurrenceDef, ROW_COLUMN, SEGMENT_COLUMN, Spec};

use carry::Carried;
use check::Checked;
use fill::append;
pub use plan::table_columns;
use plan::{Candidate, ElementPlan, Plans, column_type};

/// A column value of a row being collected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
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
struct Row {
    ordinal: usize,
    segment: usize,
    parents: Vec<Option<usize>>,
    cells: Vec<Slot>,
    /// Per column: how many segments have matched its source, for a column
    /// that picks the n-th.
    seen: Vec<usize>,
    bytes: Vec<u8>,
}

/// What the projector keeps for one table.
#[derive(Debug, Clone)]
struct TableState {
    /// Column types of the declared columns, in order.
    kinds: Vec<ColumnType>,
    /// Number of the next row.
    next: usize,
    /// `true` while an instance of an anchor loop is open (tables without
    /// `segment` only).
    open: bool,
    row: Row,
    /// The values of the columns that read a loop above the anchor, kept
    /// from the enclosing instance of that loop until a row opens.
    carried: Vec<Carried>,
    table: Table,
}

/// Turns the engine's events and the segments they name into table rows and
/// element diagnostics, one segment at a time.
#[derive(Debug, Clone)]
pub struct Projector<'s> {
    spec: &'s Spec,
    separator: u8,
    tables: Vec<TableState>,
    /// Per loop: the table without `segment` anchored in it.
    anchored: Vec<Option<usize>>,
    /// Per segment id: what is done with the segment when it is captured.
    plans: Plans<'s>,
    /// Per loop: the tables anchored on a segment inside it.
    segment_tables: Vec<Vec<usize>>,
    /// Per loop: `(table, carried value)` for the values read from that
    /// loop, which start over when an instance of it opens.
    resets: Vec<Vec<(usize, usize)>>,
    /// Open loop instances with their ordinals, outermost first.
    open: Vec<(LoopId, usize)>,
    /// Instances opened so far, per loop index.
    ordinals: Vec<usize>,
    checked: Vec<Checked>,
    /// The occurrence the segment being checked takes in its loop, when it
    /// has code lists of its own, which replace its elements' lists.
    occurrence: Option<&'s OccurrenceDef>,
    /// Index of the occurrence the latest captured segment matched in its
    /// loop's occurrences.
    matched: Option<usize>,
    /// Cells of the row being appended, kept for their allocation.
    cells: Vec<Cell<'static>>,
    joined: Vec<u8>,
    diagnostics: Vec<Diagnostic>,
}

impl<'s> Projector<'s> {
    /// A projector at the root with empty tables. `delimiters` gives the
    /// component separator, which an element read as one text keeps.
    pub fn new(spec: &'s Spec, delimiters: &Delimiters) -> Self {
        let loops = spec.loops().len();
        let mut anchored = vec![None; loops];
        let mut plans = Plans::default();
        for (id, def) in spec.segments() {
            plans.entry(id, loops).elements = def
                .elements
                .iter()
                .map(|(&position, element)| ElementPlan::new(position, element))
                .collect();
        }
        for (at, def) in spec.loops().iter().enumerate() {
            for (index, occurrence) in def.occurrences.iter().enumerate() {
                let plan = plans.entry(&occurrence.segment, loops);
                if let Some(list) = plan.occurrences.get_mut(at) {
                    list.push(Candidate {
                        index,
                        qualifier: occurrence.qualifier.as_ref(),
                        own_codes: (!occurrence.codes.is_empty()).then_some(occurrence),
                    });
                }
            }
        }
        let mut segment_tables = vec![Vec::new(); loops];
        let mut resets = vec![Vec::new(); loops];
        let mut tables = Vec::with_capacity(spec.tables().len());
        for (index, def) in spec.tables().iter().enumerate() {
            let kinds: Vec<ColumnType> = def
                .columns
                .iter()
                .map(|(_, source)| column_type(spec, def, source))
                .collect();
            let index_column = ColumnType::Int64 { scale: 0 };
            let mut columns = vec![
                (ROW_COLUMN.to_string(), index_column),
                (SEGMENT_COLUMN.to_string(), index_column),
            ];
            for &above in &def.ancestors {
                columns.push((spec.tables()[above].reference.clone(), index_column));
            }
            columns.extend(
                def.columns
                    .iter()
                    .map(|(name, _)| name.clone())
                    .zip(kinds.iter().copied()),
            );
            for &id in &def.loops {
                match def.segment {
                    Some(_) => segment_tables[id.index()].push(index),
                    None => anchored[id.index()] = Some(index),
                }
            }
            let carried = if def.segment.is_none() {
                plans.watch_columns(spec, index, &mut resets)
            } else {
                Vec::new()
            };
            plans.mark_columns(def);
            tables.push(TableState {
                kinds,
                next: 0,
                open: false,
                row: Row::default(),
                carried,
                table: Table::new(def.name.clone(), columns),
            });
        }
        Self {
            spec,
            separator: delimiters.component,
            tables,
            anchored,
            plans,
            segment_tables,
            resets,
            open: Vec::new(),
            ordinals: vec![0; loops],
            checked: Vec::new(),
            occurrence: None,
            matched: None,
            cells: Vec::new(),
            joined: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Consumes the events the engine returned for `segment` and returns the
    /// diagnostics its elements raise. The slice is valid until the next call.
    pub fn on(&mut self, segment: &Segment<'_>, events: &[Event]) -> &[Diagnostic] {
        self.diagnostics.clear();
        self.matched = None;
        for &event in events {
            match event {
                Event::LoopOpened {
                    id,
                    segment: trigger,
                    ..
                } => self.opened(id, trigger),
                Event::Captured { id, .. } => self.captured(id, segment),
                Event::LoopClosed { .. } => self.closed(),
                Event::Unmatched { .. } | Event::Empty { .. } => {}
            }
        }
        &self.diagnostics
    }

    /// Closes every loop still open, as the engine's `finish` does, appending
    /// the rows they were collecting. The projector is then back at the
    /// root and its row numbers restart at 0, so the tables should be taken
    /// before it is fed a new stream.
    pub fn finish(&mut self) -> &[Diagnostic] {
        self.diagnostics.clear();
        while !self.open.is_empty() {
            self.closed();
        }
        self.ordinals.iter_mut().for_each(|count| *count = 0);
        for state in &mut self.tables {
            state.next = 0;
            state.carried.iter_mut().for_each(Carried::clear);
        }
        &self.diagnostics
    }

    /// Moves every appended row out, leaving the tables empty. Rows still
    /// being collected stay, and keep their numbers. A text column holds at
    /// most `i32::MAX` bytes until it is drained; draining per transaction
    /// stays far below that (see the module notes for what happens past it).
    pub fn take_tables(&mut self) -> Tables {
        Tables::new(
            self.tables
                .iter_mut()
                .map(|state| state.table.take_rows())
                .collect(),
        )
    }

    fn opened(&mut self, id: LoopId, trigger: usize) {
        let ordinal = self.ordinals[id.index()].saturating_add(1);
        self.ordinals[id.index()] = ordinal;
        self.open.push((id, ordinal));
        self.reset_carried(id);
        let Some(index) = self.anchored[id.index()] else {
            return;
        };
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

    fn closed(&mut self) {
        let Some(&(id, _)) = self.open.last() else {
            return;
        };
        if let Some(index) = self.anchored[id.index()] {
            let state = &mut self.tables[index];
            if state.open {
                state.open = false;
                let dropped = append(&mut state.table, &state.row, &mut self.cells);
                let segment = state.row.segment;
                self.report_dropped(dropped, index, segment);
            }
        }
        self.open.pop();
    }

    fn captured(&mut self, id: LoopId, segment: &Segment<'_>) {
        let mut joined = std::mem::take(&mut self.joined);
        let plans = std::mem::take(&mut self.plans);
        let plan = plans.get(segment.id);
        // Every candidate has the segment's id: only its qualifier is left
        // to check.
        let found = plan
            .and_then(|plan| plan.occurrences.get(id.index()))
            .and_then(|candidates| {
                candidates.iter().find(|candidate| {
                    candidate
                        .qualifier
                        .is_none_or(|qualifier| qualifier.matches(segment))
                })
            });
        self.matched = found.map(|candidate| candidate.index);
        self.occurrence = found.and_then(|candidate| candidate.own_codes);
        self.check(
            plan.map_or(&[], |plan| &plan.elements),
            segment,
            &mut joined,
        );
        if let Some(watchers) = plan.and_then(|plan| plan.watchers.get(id.index())) {
            self.fill(watchers, segment, &mut joined);
        }
        self.plans = plans;
        self.segment_rows(id, segment, &mut joined);
        self.joined = joined;
    }

    /// Index of the occurrence the segment of the latest `on` matched in the
    /// loop that captured it; `None` when nothing was captured or nothing
    /// matched.
    pub(crate) fn matched(&self) -> Option<usize> {
        self.matched
    }

    /// The diagnostics of the latest `on` or `finish`.
    pub(crate) fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// The number of the row `table` is collecting, if an instance is open.
    fn open_row(&self, table: usize) -> Option<usize> {
        let state = &self.tables[table];
        state.open.then_some(state.row.ordinal)
    }
}
