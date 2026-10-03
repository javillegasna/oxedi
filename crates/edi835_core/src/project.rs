//! Projection of a segment stream into typed tables, with element checks.
//!
//! The projector follows the loops the engine opens and closes and fills the
//! tables of the spec. A table without a `segment` gets one row per instance
//! of its anchor loops: the row number is taken when the instance opens, each
//! column fills from the first captured segment that matches its source while
//! the instance is open, and the row is appended when the instance closes. A
//! table anchored on a segment gets its rows when that segment is captured,
//! one per element group when the segment repeats a group. Every row also
//! carries its number, the index of its anchor segment and, for each table
//! above it, the number of that table's open row (null when none is open).
//! Row numbers count from 0 across the whole stream, so they keep their
//! meaning when the tables are drained part way.
//!
//! Every captured segment the spec defines is checked element by element as
//! it arrives: required elements, types, lengths and composite shapes. An
//! element defined without components is read as one text, with any
//! component separator it contains kept in place; components are read only
//! where the definition declares them. A column reads the value the check
//! already parsed, so no element is parsed twice. A value that is missing or
//! does not parse as its type is null in its column; a value whose length is
//! out of range is reported and kept.
//!
//! Allocation: appending a row collects its cells into a new vector and grows
//! the table's buffers; each diagnostic owns its text. A row being collected
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

use crate::column::{
    Cell, CellError, ColumnType, RowError, Table, Tables, parse_dt, parse_n, parse_r, parse_tm,
};
use crate::delimiters::Delimiters;
use crate::diagnostic::{Diagnostic, LoopRef, Rule};
use crate::element::Element;
use crate::engine::Event;
use crate::segment::Segment;
use crate::spec::{
    ColumnSource, ElementDef, ElementType, LoopId, ROW_COLUMN, SEGMENT_COLUMN, Spec, TableDef,
};

/// A value an element check parsed, or a column read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Parsed {
    /// Absent, empty, or not a valid value of its type.
    Null,
    /// Text, to be copied from the segment.
    Text,
    Int(i64),
    Decimal(i128),
    Date(i32),
    Time(i32),
}

/// One defined element (or component) of the current segment, checked.
#[derive(Debug, Clone, Copy)]
struct Checked {
    element: usize,
    component: Option<usize>,
    /// The column type its definition maps to.
    kind: ColumnType,
    value: Parsed,
}

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
    /// Per loop: `(table, column)` for the columns that read segments captured in it.
    watchers: Vec<Vec<(usize, usize)>>,
    /// Per loop: the tables anchored on a segment inside it.
    segment_tables: Vec<Vec<usize>>,
    /// Open loop instances with their ordinals, outermost first.
    open: Vec<(LoopId, usize)>,
    /// Instances opened so far, per loop index.
    ordinals: Vec<usize>,
    checked: Vec<Checked>,
    joined: Vec<u8>,
    diagnostics: Vec<Diagnostic>,
}

impl<'s> Projector<'s> {
    /// A projector at the root with empty tables. `delimiters` gives the
    /// component separator, which an element read as one text keeps.
    pub fn new(spec: &'s Spec, delimiters: &Delimiters) -> Self {
        let loops = spec.loops().len();
        let mut anchored = vec![None; loops];
        let mut watchers = vec![Vec::new(); loops];
        let mut segment_tables = vec![Vec::new(); loops];
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
            if def.segment.is_none() {
                for (column, (_, source)) in def.columns.iter().enumerate() {
                    let reader = match source {
                        ColumnSource::Element { loop_id, .. }
                        | ColumnSource::SegmentIndex { loop_id, .. } => *loop_id,
                        ColumnSource::GroupElement { .. } => continue,
                    };
                    match reader {
                        Some(id) => watchers[id.index()].push((index, column)),
                        None => {
                            for &id in &def.loops {
                                watchers[id.index()].push((index, column));
                            }
                        }
                    }
                }
            }
            tables.push(TableState {
                kinds,
                next: 0,
                open: false,
                row: Row::default(),
                table: Table::new(def.name.clone(), columns),
            });
        }
        Self {
            spec,
            separator: delimiters.component,
            tables,
            anchored,
            watchers,
            segment_tables,
            open: Vec::new(),
            ordinals: vec![0; loops],
            checked: Vec::new(),
            joined: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Consumes the events the engine returned for `segment` and returns the
    /// diagnostics its elements raise. The slice is valid until the next call.
    pub fn on(&mut self, segment: &Segment<'_>, events: &[Event]) -> &[Diagnostic] {
        self.diagnostics.clear();
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
        self.tables.iter_mut().for_each(|state| state.next = 0);
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
        row.bytes.clear();
        row.segment = trigger;
        let state = &mut self.tables[index];
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
                let dropped = append(&mut state.table, &state.row);
                let segment = state.row.segment;
                self.report_dropped(dropped, index, segment);
            }
        }
        self.open.pop();
    }

    fn captured(&mut self, id: LoopId, segment: &Segment<'_>) {
        let mut joined = std::mem::take(&mut self.joined);
        self.check(segment, &mut joined);
        self.fill(id, segment, &mut joined);
        self.segment_rows(id, segment, &mut joined);
        self.joined = joined;
    }

    /// The number of the row `table` is collecting, if an instance is open.
    fn open_row(&self, table: usize) -> Option<usize> {
        let state = &self.tables[table];
        state.open.then_some(state.row.ordinal)
    }

    /// Checks every element the spec defines for the segment and keeps the
    /// parsed values for the columns that read them.
    fn check(&mut self, segment: &Segment<'_>, joined: &mut Vec<u8>) {
        self.checked.clear();
        let spec = self.spec;
        let Some(def) = spec.segment(segment.id) else {
            return;
        };
        for (&position, element) in &def.elements {
            if element.composite.is_empty() {
                let text = leaf_text(segment, position, None, self.separator, joined);
                let value = self.check_value(segment, position, None, element, text);
                self.checked.push(Checked {
                    element: position,
                    component: None,
                    kind: ColumnType::of(Some(element.kind)),
                    value,
                });
                continue;
            }
            let parts: &[std::borrow::Cow<'_, [u8]>] = match segment.element(position) {
                Some(Element::Composite(parts)) => parts,
                Some(Element::Simple(value)) => std::slice::from_ref(value),
                None => &[],
            };
            if parts.iter().all(|part| part.is_empty()) {
                if element.required {
                    self.report(
                        Rule::RequiredElementMissing {
                            segment_id: segment.id.to_vec(),
                            element: position,
                            component: None,
                            name: element.name.clone(),
                        },
                        segment.index,
                        position,
                        None,
                        &[],
                    );
                }
                continue;
            }
            let declared = element.composite.keys().copied().max().unwrap_or_default();
            if parts.len() > declared {
                let extra = parts.get(declared).map_or(&[][..], |part| part.as_ref());
                self.report(
                    Rule::CompositeShape {
                        segment_id: segment.id.to_vec(),
                        element: position,
                        name: element.name.clone(),
                        declared,
                        found: parts.len(),
                    },
                    segment.index,
                    position,
                    declared.checked_add(1),
                    extra,
                );
            }
            for (&component, def) in &element.composite {
                let text = component
                    .checked_sub(1)
                    .and_then(|at| parts.get(at))
                    .map_or(&[][..], |part| part.as_ref());
                let value = self.check_value(segment, position, Some(component), def, text);
                self.checked.push(Checked {
                    element: position,
                    component: Some(component),
                    kind: ColumnType::of(Some(def.kind)),
                    value,
                });
            }
        }
    }

    /// Checks one value against its definition, reports what fails, and
    /// returns the parsed value (null when missing or of the wrong type).
    fn check_value(
        &mut self,
        segment: &Segment<'_>,
        element: usize,
        component: Option<usize>,
        def: &ElementDef,
        text: &[u8],
    ) -> Parsed {
        if text.is_empty() {
            if def.required {
                self.report(
                    Rule::RequiredElementMissing {
                        segment_id: segment.id.to_vec(),
                        element,
                        component,
                        name: def.name.clone(),
                    },
                    segment.index,
                    element,
                    component,
                    &[],
                );
            }
            return Parsed::Null;
        }
        let Some(value) = parse(ColumnType::of(Some(def.kind)), text) else {
            self.report(
                Rule::TypeMismatch {
                    segment_id: segment.id.to_vec(),
                    element,
                    component,
                    name: def.name.clone(),
                    expected: def.kind,
                },
                segment.index,
                element,
                component,
                text,
            );
            return Parsed::Null;
        };
        // Numeric lengths count digits only, as X12 does: no sign, no point.
        let length = match def.kind {
            ElementType::N(_) | ElementType::R { .. } => {
                text.iter().filter(|byte| byte.is_ascii_digit()).count()
            }
            _ => text.len(),
        };
        if def.min.is_some_and(|min| length < min) || def.max.is_some_and(|max| length > max) {
            self.report(
                Rule::LengthOutOfRange {
                    segment_id: segment.id.to_vec(),
                    element,
                    component,
                    name: def.name.clone(),
                    min: def.min,
                    max: def.max,
                    length,
                },
                segment.index,
                element,
                component,
                text,
            );
        }
        value
    }

    /// Fills the open rows whose columns read this segment and have no value yet.
    fn fill(&mut self, id: LoopId, segment: &Segment<'_>, joined: &mut Vec<u8>) {
        let spec = self.spec;
        for &(index, column) in &self.watchers[id.index()] {
            let state = &mut self.tables[index];
            if !state.open || state.row.cells.get(column) != Some(&Slot::Unset) {
                continue;
            }
            let Some((_, source)) = spec.tables()[index].columns.get(column) else {
                continue;
            };
            let slot = match source {
                ColumnSource::Element {
                    segment: wanted,
                    conditions,
                    element,
                    component,
                    ..
                } => {
                    if !matches(segment, wanted, conditions) {
                        continue;
                    }
                    let kind = state
                        .kinds
                        .get(column)
                        .copied()
                        .unwrap_or(ColumnType::Binary);
                    let at = Place {
                        element: *element,
                        component: *component,
                        kind,
                    };
                    read(
                        &self.checked,
                        segment,
                        at,
                        self.separator,
                        joined,
                        &mut state.row.bytes,
                    )
                }
                ColumnSource::SegmentIndex {
                    segment: wanted,
                    conditions,
                    ..
                } => {
                    if !matches(segment, wanted, conditions) {
                        continue;
                    }
                    i64::try_from(segment.index).map_or(Slot::Null, Slot::Int)
                }
                ColumnSource::GroupElement { .. } => continue,
            };
            if let Some(cell) = state.row.cells.get_mut(column) {
                *cell = slot;
            }
        }
    }

    /// Appends the rows of the tables anchored on this segment.
    fn segment_rows(&mut self, id: LoopId, segment: &Segment<'_>, joined: &mut Vec<u8>) {
        let spec = self.spec;
        for slot in 0..self.segment_tables[id.index()].len() {
            let index = self.segment_tables[id.index()][slot];
            let def = &spec.tables()[index];
            if def.segment.as_deref() != Some(segment.id) {
                continue;
            }
            let Some(repeat) = def.repeat else {
                self.segment_row(index, segment, None, joined);
                continue;
            };
            let mut start = Some(repeat.from);
            while let Some(position) = start.filter(|&at| at <= segment.elements.len()) {
                if segment.element(position).is_some_and(has_content) {
                    self.segment_row(index, segment, Some(position), joined);
                }
                start = position.checked_add(repeat.step);
            }
        }
    }

    /// Appends one row of a table anchored on a segment; `group` is the
    /// position of the first element of the row's group.
    fn segment_row(
        &mut self,
        index: usize,
        segment: &Segment<'_>,
        group: Option<usize>,
        joined: &mut Vec<u8>,
    ) {
        let spec = self.spec;
        let def = &spec.tables()[index];
        let mut row = std::mem::take(&mut self.tables[index].row);
        row.parents.clear();
        row.parents
            .extend(def.ancestors.iter().map(|&above| self.open_row(above)));
        row.cells.clear();
        row.bytes.clear();
        for (column, (_, source)) in def.columns.iter().enumerate() {
            let kind = self.tables[index]
                .kinds
                .get(column)
                .copied()
                .unwrap_or(ColumnType::Binary);
            let at = match source {
                ColumnSource::Element {
                    element, component, ..
                } => Some((*element, *component)),
                ColumnSource::GroupElement { offset, component } => group
                    .and_then(|start| start.checked_add(*offset))
                    .map(|element| (element, *component)),
                ColumnSource::SegmentIndex { .. } => {
                    row.cells
                        .push(i64::try_from(segment.index).map_or(Slot::Null, Slot::Int));
                    continue;
                }
            };
            let slot = match at {
                Some((element, component)) => read(
                    &self.checked,
                    segment,
                    Place {
                        element,
                        component,
                        kind,
                    },
                    self.separator,
                    joined,
                    &mut row.bytes,
                ),
                None => Slot::Null,
            };
            row.cells.push(slot);
        }
        let state = &mut self.tables[index];
        row.ordinal = state.next;
        row.segment = segment.index;
        state.next = state.next.saturating_add(1);
        let dropped = append(&mut state.table, &row);
        state.row = row;
        self.report_dropped(dropped, index, segment.index);
    }

    /// Reports each text value an append could not store.
    fn report_dropped(&mut self, dropped: Vec<(String, usize)>, table: usize, segment: usize) {
        for (column, bytes) in dropped {
            let rule = Rule::ValueDropped {
                table: self.spec.tables()[table].name.clone(),
                column,
                bytes,
            };
            self.push_diagnostic(rule, segment, None, None, &[]);
        }
    }

    fn report(
        &mut self,
        rule: Rule,
        segment: usize,
        element: usize,
        component: Option<usize>,
        datum: &[u8],
    ) {
        self.push_diagnostic(rule, segment, Some(element), component, datum);
    }

    fn push_diagnostic(
        &mut self,
        rule: Rule,
        segment: usize,
        element: Option<usize>,
        component: Option<usize>,
        datum: &[u8],
    ) {
        let spec = self.spec;
        let path = self
            .open
            .iter()
            .map(|&(id, ordinal)| LoopRef {
                name: spec.loop_name(id).to_string(),
                ordinal,
            })
            .collect();
        self.diagnostics.push(Diagnostic::new(
            rule,
            Some(segment),
            element,
            component,
            path,
            datum.to_vec(),
        ));
    }
}

/// The column type of a declared column: its element's type, `Int64` for a
/// segment index, `Binary` for an element the spec does not define.
fn column_type(spec: &Spec, table: &TableDef, source: &ColumnSource) -> ColumnType {
    let def = match source {
        ColumnSource::Element {
            segment,
            element,
            component,
            ..
        } => spec.element_def(segment, *element, *component),
        ColumnSource::SegmentIndex { .. } => return ColumnType::Int64 { scale: 0 },
        ColumnSource::GroupElement { offset, component } => match (&table.segment, table.repeat) {
            (Some(segment), Some(repeat)) => repeat
                .from
                .checked_add(*offset)
                .and_then(|position| spec.element_def(segment, position, *component)),
            _ => None,
        },
    };
    ColumnType::of(def.map(|def| def.kind))
}

/// `true` when the segment has the id and every condition holds.
fn matches(segment: &Segment<'_>, id: &[u8], conditions: &[(usize, Vec<u8>)]) -> bool {
    segment.id == id
        && conditions.iter().all(|(position, value)| {
            segment.element(*position).and_then(Element::simple) == Some(value.as_slice())
        })
}

/// `true` when an element has a non-empty value or component.
fn has_content(element: &Element<'_>) -> bool {
    match element {
        Element::Simple(value) => !value.is_empty(),
        Element::Composite(parts) => parts.iter().any(|part| !part.is_empty()),
    }
}

/// The text of an element, or of one of its components. An element read
/// whole that the file split into components is written back into
/// `joined` with the component separator between them; component 1 of a
/// simple element is the element itself.
fn leaf_text<'a>(
    segment: &'a Segment<'_>,
    element: usize,
    component: Option<usize>,
    separator: u8,
    joined: &'a mut Vec<u8>,
) -> &'a [u8] {
    match (segment.element(element), component) {
        (None, _) | (Some(Element::Simple(_)), Some(2..)) => &[],
        (Some(Element::Simple(value)), _) => value,
        (Some(Element::Composite(parts)), Some(component)) => component
            .checked_sub(1)
            .and_then(|at| parts.get(at))
            .map_or(&[][..], |part| part.as_ref()),
        (Some(Element::Composite(parts)), None) => {
            joined.clear();
            for (at, part) in parts.iter().enumerate() {
                if at > 0 {
                    joined.push(separator);
                }
                joined.extend_from_slice(part);
            }
            joined
        }
    }
}

/// A non-empty text as a value of the column type; `None` when it does not parse.
fn parse(kind: ColumnType, text: &[u8]) -> Option<Parsed> {
    Some(match kind {
        ColumnType::Binary => Parsed::Text,
        ColumnType::Int64 { .. } => Parsed::Int(parse_n(text)?),
        ColumnType::Decimal128 { scale, .. } => Parsed::Decimal(parse_r(text, scale)?),
        ColumnType::Date32 => Parsed::Date(parse_dt(text)?),
        ColumnType::Time32 => Parsed::Time(parse_tm(text)?),
    })
}

/// Where a column reads inside a segment, and as what.
#[derive(Debug, Clone, Copy)]
struct Place {
    element: usize,
    component: Option<usize>,
    kind: ColumnType,
}

/// The column value at `at`: the checked value when the definition there
/// maps to the column's type, otherwise the text parsed as that type (an
/// element the spec does not define, or a group whose definition differs).
/// Text is copied into `bytes`.
fn read(
    checked: &[Checked],
    segment: &Segment<'_>,
    at: Place,
    separator: u8,
    joined: &mut Vec<u8>,
    bytes: &mut Vec<u8>,
) -> Slot {
    let known = checked
        .iter()
        .find(|c| c.element == at.element && c.component == at.component && c.kind == at.kind);
    let value = match known {
        Some(checked) => checked.value,
        None => {
            let text = leaf_text(segment, at.element, at.component, separator, joined);
            if text.is_empty() {
                Parsed::Null
            } else {
                parse(at.kind, text).unwrap_or(Parsed::Null)
            }
        }
    };
    match value {
        Parsed::Null => Slot::Null,
        Parsed::Text => {
            let text = leaf_text(segment, at.element, at.component, separator, joined);
            let start = bytes.len();
            bytes.extend_from_slice(text);
            Slot::Bytes(start, bytes.len())
        }
        Parsed::Int(value) => Slot::Int(value),
        Parsed::Decimal(value) => Slot::Decimal(value),
        Parsed::Date(value) => Slot::Date(value),
        Parsed::Time(value) => Slot::Time(value),
    }
}

/// Appends a collected row: its number, its anchor segment, the rows above
/// it, then its columns; a column no segment matched is null. A text value
/// that would take its column past what `i32` offsets address is stored as
/// null and returned with its column name and the byte total it would have
/// produced, so row numbers stay aligned and nothing disappears unreported.
fn append(table: &mut Table, row: &Row) -> Vec<(String, usize)> {
    let index = |value: usize| i64::try_from(value).map_or(Cell::Null, Cell::Int64);
    let mut cells = Vec::with_capacity(2 + row.parents.len() + row.cells.len());
    cells.push(index(row.ordinal));
    cells.push(index(row.segment));
    cells.extend(
        row.parents
            .iter()
            .map(|parent| parent.map_or(Cell::Null, index)),
    );
    cells.extend(row.cells.iter().map(|slot| match *slot {
        Slot::Unset | Slot::Null => Cell::Null,
        Slot::Bytes(start, end) => row.bytes.get(start..end).map_or(Cell::Null, Cell::Binary),
        Slot::Int(value) => Cell::Int64(value),
        Slot::Decimal(value) => Cell::Decimal128(value),
        Slot::Date(value) => Cell::Date32(value),
        Slot::Time(value) => Cell::Time32(value),
    }));
    let mut dropped = Vec::new();
    // Every cell has its column's type, so only text can fail to fit; each
    // retry nulls one cell, and nulls always fit.
    while let Err(error) = table.push_row(&cells) {
        let RowError::Cell {
            column,
            source: CellError::BinaryOverflow { bytes },
            ..
        } = error
        else {
            break;
        };
        let at = table.columns().iter().position(|(name, _)| *name == column);
        match at.and_then(|at| cells.get_mut(at)) {
            Some(cell) => *cell = Cell::Null,
            None => break,
        }
        dropped.push((column, bytes));
    }
    dropped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LoopEngine, Tokenizer};
    use proptest::prelude::*;

    const SPEC: &str = r#"{"name":"t",
        "loops":{
            "head":{"trigger":{"segment":"HD"},"segments":["ZZ"],"end":"TR"},
            "note":{"parent":"head","trigger":{"segment":"NM","where":{"1":"P"}}},
            "claim":{"parent":"head","trigger":{"segment":"CL"},"segments":["DT","AJ","RF"]},
            "line":{"parent":"claim","trigger":{"segment":"LN"},"segments":["DT","AJ"]}
        },
        "segments":{
            "HD":{"elements":{"1":{"name":"batch","type":"AN","required":true,"min":1,"max":5}}},
            "CL":{"elements":{
                "1":{"name":"claim_id","type":"AN","required":true,"min":1,"max":10},
                "2":{"name":"charge","type":"R","required":true,"max":10},
                "3":{"name":"units","type":"N0","max":2},
                "4":{"name":"procedure","type":"AN","composite":{
                    "1":{"name":"qualifier","type":"ID","required":true,"min":2,"max":2},
                    "2":{"name":"code","type":"AN","required":true,"max":5}
                }}
            }},
            "DT":{"elements":{
                "1":{"name":"qualifier","type":"ID","required":true},
                "2":{"name":"date","type":"DT"},
                "3":{"name":"time","type":"TM"}
            }},
            "AJ":{"elements":{
                "1":{"name":"group","type":"ID","required":true},
                "2":{"name":"reason","type":"ID"},
                "3":{"name":"amount","type":"R"},
                "4":{"name":"reason_2","type":"ID"},
                "5":{"name":"amount_2","type":"R"}
            }}
        },
        "tables":{
            "heads":{"loops":["head"],"ref":"head","columns":{
                "batch":{"segment":"HD","element":1},
                "payer":{"loop":"note","segment":"NM","element":2}
            }},
            "claims":{"loops":["claim"],"ref":"claim","columns":{
                "claim_id":{"segment":"CL","element":1},
                "charge":{"segment":"CL","element":2},
                "units":{"segment":"CL","element":3},
                "procedure":{"segment":"CL","element":4},
                "code":{"segment":"CL","element":4,"component":2},
                "from":{"segment":"DT","where":{"1":"150"},"element":2},
                "reference":{"segment":"RF","element":2},
                "first_line_at":{"loop":"line","segment":"LN","segment_index":true}
            }},
            "lines":{"loops":["line"],"ref":"line","columns":{
                "code":{"segment":"LN","element":1},
                "date":{"segment":"DT","element":2},
                "time":{"segment":"DT","element":3}
            }},
            "adjustments":{"loops":["claim","line"],"segment":"AJ","repeat":{"from":2,"step":2},"columns":{
                "group":{"element":1},
                "reason":{"group_element":0},
                "amount":{"group_element":1}
            }}
        }
    }"#;

    fn spec() -> Spec {
        Spec::from_json(SPEC).unwrap()
    }

    fn delimiters() -> Delimiters {
        Delimiters::new(b'*', b':', b'~')
    }

    /// Runs the engine and a projector over `input`, `finish` included.
    fn project(spec: &Spec, input: &str) -> (Tables, Vec<Diagnostic>) {
        let mut engine = LoopEngine::new(spec);
        let mut projector = Projector::new(spec, &delimiters());
        let mut diagnostics = Vec::new();
        for segment in Tokenizer::with_delimiters(input.as_bytes(), delimiters()) {
            let events = engine.feed(&segment);
            diagnostics.extend_from_slice(projector.on(&segment, events));
        }
        engine.finish();
        diagnostics.extend_from_slice(projector.finish());
        (projector.take_tables(), diagnostics)
    }

    /// A table as text: the column names, then one line per row.
    fn rows(tables: &Tables, name: &str) -> Vec<String> {
        let table = tables.get(name).unwrap();
        let header = table
            .columns()
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>()
            .join(" | ");
        std::iter::once(header)
            .chain((0..table.len()).map(|row| {
                table
                    .columns()
                    .iter()
                    .map(|(_, column)| column.render(row).unwrap())
                    .collect::<Vec<_>>()
                    .join(" | ")
            }))
            .collect()
    }

    fn rendered(diagnostics: &[Diagnostic]) -> Vec<String> {
        diagnostics.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn columns_fill_from_the_first_matching_segment_and_rows_close_with_their_loop() {
        let spec = spec();
        let (tables, diagnostics) = project(
            &spec,
            "HD*B1~NM*P*ACME~NM*P*OTHER~CL*C1*12.5*3*HC:99213~DT*151*20240101~DT*150*20240105~DT*150*20240106~RF*Q*X1~LN*L1~DT*472*20240107*1230~TR~",
        );
        assert_eq!(rendered(&diagnostics), Vec::<String>::new());
        assert_eq!(
            rows(&tables, "heads"),
            vec!["row | segment | batch | payer", "0 | 0 | B1 | ACME"]
        );
        assert_eq!(
            rows(&tables, "claims"),
            vec![
                "row | segment | head | charge | claim_id | code | first_line_at | from | procedure | reference | units",
                "0 | 3 | 0 | 12.50 | C1 | 99213 | 8 | 2024-01-05 | HC:99213 | X1 | 3",
            ]
        );
        assert_eq!(
            rows(&tables, "lines"),
            vec![
                "row | segment | head | claim | code | date | time",
                "0 | 8 | 0 | 0 | L1 | 2024-01-07 | 12:30:00",
            ]
        );
    }

    #[test]
    fn a_text_value_past_the_column_limit_is_reported_and_its_cell_is_null() {
        let spec = spec();
        crate::column::OFFSET_LIMIT.with(|limit| limit.set(6));
        let (tables, diagnostics) = project(&spec, "HD*ABCD~TR~HD*EFGH~TR~");
        crate::column::OFFSET_LIMIT.with(|limit| limit.set(i32::MAX as usize));
        assert_eq!(
            rows(&tables, "heads"),
            vec![
                "row | segment | batch | payer",
                "0 | 0 | ABCD | ∅",
                "1 | 2 | ∅ | ∅"
            ],
            "the row is kept, with the cell null"
        );
        assert_eq!(
            rendered(&diagnostics),
            vec![
                "SNIP 2 · text for column \"batch\" of table \"heads\" was not stored: it would bring the column to 8 bytes and a column holds at most 2147483647 · segment #2 · at head#2 · datum \"\""
            ]
        );
    }

    #[test]
    fn a_text_value_past_the_limit_in_a_segment_row_is_reported() {
        let spec = spec();
        crate::column::OFFSET_LIMIT.with(|limit| limit.set(3));
        let (tables, diagnostics) = project(&spec, "HD*A~CL*C*1~AJ*CO*X*1~AJ*CO*Y*2~TR~");
        crate::column::OFFSET_LIMIT.with(|limit| limit.set(i32::MAX as usize));
        assert_eq!(
            rows(&tables, "adjustments")[1..],
            [
                "0 | 2 | 0 | 0 | ∅ | 1.00 | CO | X",
                "1 | 3 | 0 | 0 | ∅ | 2.00 | ∅ | Y"
            ]
        );
        assert_eq!(
            rendered(&diagnostics),
            vec![
                "SNIP 2 · text for column \"group\" of table \"adjustments\" was not stored: it would bring the column to 4 bytes and a column holds at most 2147483647 · segment #3 · at head#1/claim#1 · datum \"\""
            ]
        );
    }

    #[test]
    fn a_column_no_segment_matches_is_null() {
        let spec = spec();
        let (tables, _) = project(&spec, "HD*B1~CL*C1*1~TR~");
        assert_eq!(
            rows(&tables, "claims")[1],
            "0 | 1 | 0 | 1.00 | C1 | ∅ | ∅ | ∅ | ∅ | ∅ | ∅"
        );
        assert_eq!(rows(&tables, "heads")[1], "0 | 0 | B1 | ∅");
    }

    #[test]
    fn every_row_names_the_open_row_of_each_table_above_it() {
        let spec = spec();
        let (tables, _) = project(
            &spec,
            "HD*B1~CL*C1*1~AJ*CO*45*10~LN*L1~AJ*PR*1*2~LN*L2~CL*C2*2~LN*L3~AJ*OA*3*4*5*6~TR~",
        );
        assert_eq!(
            rows(&tables, "lines"),
            vec![
                "row | segment | head | claim | code | date | time",
                "0 | 3 | 0 | 0 | L1 | ∅ | ∅",
                "1 | 5 | 0 | 0 | L2 | ∅ | ∅",
                "2 | 7 | 0 | 1 | L3 | ∅ | ∅",
            ]
        );
        assert_eq!(
            rows(&tables, "adjustments"),
            vec![
                "row | segment | head | claim | line | amount | group | reason",
                "0 | 2 | 0 | 0 | ∅ | 10.00 | CO | 45",
                "1 | 4 | 0 | 0 | 0 | 2.00 | PR | 1",
                "2 | 8 | 0 | 1 | 2 | 4.00 | OA | 3",
                "3 | 8 | 0 | 1 | 2 | 6.00 | OA | 5",
            ]
        );
    }

    #[test]
    fn a_repeated_group_without_its_first_element_gives_no_row() {
        let spec = spec();
        let (tables, _) = project(&spec, "HD*B1~CL*C1*1~AJ*CO*45*10**7~AJ*PR~TR~");
        assert_eq!(
            rows(&tables, "adjustments"),
            vec![
                "row | segment | head | claim | line | amount | group | reason",
                "0 | 2 | 0 | 0 | ∅ | 10.00 | CO | 45",
            ]
        );
    }

    #[test]
    fn row_numbers_keep_counting_across_drains() {
        let spec = spec();
        let mut engine = LoopEngine::new(&spec);
        let mut projector = Projector::new(&spec, &delimiters());
        let mut drains = Vec::new();
        let input = "HD*B1~CL*C1*1~LN*L1~CL*C2*2~LN*L2~TR~";
        for segment in Tokenizer::with_delimiters(input.as_bytes(), delimiters()) {
            let events = engine.feed(&segment).to_vec();
            projector.on(&segment, &events);
            if events.contains(&Event::LoopClosed {
                id: spec.loop_id("claim").unwrap(),
            }) {
                drains.push(projector.take_tables());
            }
        }
        engine.finish();
        projector.finish();
        drains.push(projector.take_tables());
        let lines: Vec<Vec<String>> = drains
            .iter()
            .map(|tables| rows(tables, "lines").split_off(1))
            .collect();
        assert_eq!(
            lines,
            vec![
                vec!["0 | 2 | 0 | 0 | L1 | ∅ | ∅".to_string()],
                vec!["1 | 4 | 0 | 1 | L2 | ∅ | ∅".to_string()],
                vec![],
            ]
        );
        assert_eq!(
            rows(&drains[1], "heads").split_off(1),
            vec!["0 | 0 | B1 | ∅"],
            "the end segment closes the head right after the last claim"
        );
        assert!(drains[2].iter().all(Table::is_empty));
    }

    #[test]
    fn finishing_appends_the_rows_still_open() {
        let spec = spec();
        let (tables, _) = project(&spec, "HD*B1~CL*C1*1~LN*L1~");
        assert_eq!(tables.get("heads").unwrap().len(), 1);
        assert_eq!(tables.get("claims").unwrap().len(), 1);
        assert_eq!(tables.get("lines").unwrap().len(), 1);
    }

    #[test]
    fn a_value_that_is_not_its_type_is_reported_and_null() {
        let spec = spec();
        let (tables, diagnostics) = project(&spec, "HD*B1~CL*C1*12A~TR~");
        assert_eq!(
            rendered(&diagnostics),
            vec![
                "SNIP 2 · element CL02 (charge) is not a valid R (decimal, scale 2) · segment #1, element 2 · at head#1/claim#1 · datum \"12A\""
            ]
        );
        assert_eq!(
            tables
                .get("claims")
                .unwrap()
                .column("charge")
                .unwrap()
                .get(0),
            Some(Cell::Null)
        );
    }

    #[test]
    fn a_decimal_with_more_places_than_its_scale_is_a_type_mismatch() {
        let spec = spec();
        let (_, diagnostics) = project(&spec, "HD*B1~CL*C1*1.234~TR~");
        assert_eq!(
            rendered(&diagnostics),
            vec![
                "SNIP 2 · element CL02 (charge) is not a valid R (decimal, scale 2) · segment #1, element 2 · at head#1/claim#1 · datum \"1.234\""
            ]
        );
    }

    #[test]
    fn an_empty_required_element_is_reported_with_its_name() {
        let spec = spec();
        let (_, diagnostics) = project(&spec, "HD*B1~CL**5~TR~");
        assert_eq!(
            rendered(&diagnostics),
            vec![
                "SNIP 2 · required element CL01 (claim_id) is missing or empty · segment #1, element 1 · at head#1/claim#1 · datum \"\""
            ]
        );
    }

    #[test]
    fn an_invalid_date_names_the_element_and_the_text() {
        let spec = spec();
        let (tables, diagnostics) = project(&spec, "HD*B1~CL*C1*1~DT*150*20240230~TR~");
        assert_eq!(
            rendered(&diagnostics),
            vec![
                "SNIP 2 · element DT02 (date) is not a valid DT (date CCYYMMDD or YYMMDD) · segment #2, element 2 · at head#1/claim#1 · datum \"20240230\""
            ]
        );
        assert_eq!(
            tables.get("claims").unwrap().column("from").unwrap().get(0),
            Some(Cell::Null),
            "the first matching DTM decides the column, even when its value is invalid"
        );
    }

    #[test]
    fn lengths_count_bytes_for_text_and_digits_for_numbers() {
        let spec = spec();
        let (tables, diagnostics) = project(
            &spec,
            "HD*B1~CL*ABCDEFGHIJK*-12345678.90*-12~CL*C2*1*123~TR~",
        );
        assert_eq!(
            rendered(&diagnostics),
            vec![
                "SNIP 2 · element CL01 (claim_id) has length 11; the spec allows 1 to 10 · segment #1, element 1 · at head#1/claim#1 · datum \"ABCDEFGHIJK\"",
                "SNIP 2 · element CL03 (units) has length 3; the spec allows at most 2 · segment #2, element 3 · at head#1/claim#2 · datum \"123\"",
            ]
        );
        let claims = tables.get("claims").unwrap();
        assert_eq!(
            claims.column("claim_id").unwrap().get(0),
            Some(Cell::Binary(b"ABCDEFGHIJK")),
            "a value out of range is reported and kept"
        );
        assert_eq!(
            claims.column("charge").unwrap().get(0),
            Some(Cell::Decimal128(-1_234_567_890))
        );
        assert_eq!(
            claims.column("units").unwrap().get(0),
            Some(Cell::Int64(-12))
        );
    }

    #[test]
    fn a_composite_with_more_components_than_declared_names_the_first_extra_one() {
        let spec = spec();
        let (_, diagnostics) = project(&spec, "HD*B1~CL*C1*1**HC:1:X~TR~");
        assert_eq!(
            rendered(&diagnostics),
            vec![
                "SNIP 2 · element CL04 (procedure) has 3 components; the spec declares 2 · segment #1, element 4, component 3 · at head#1/claim#1 · datum \"X\""
            ]
        );
    }

    #[test]
    fn components_are_checked_where_the_definition_declares_them() {
        let spec = spec();
        let (tables, diagnostics) = project(&spec, "HD*B1~CL*C1*1**HCPC~TR~");
        assert_eq!(
            rendered(&diagnostics),
            vec![
                "SNIP 2 · element CL04-1 (qualifier) has length 4; the spec allows 2 to 2 · segment #1, element 4, component 1 · at head#1/claim#1 · datum \"HCPC\"",
                "SNIP 2 · required element CL04-2 (code) is missing or empty · segment #1, element 4, component 2 · at head#1/claim#1 · datum \"\"",
            ]
        );
        assert_eq!(
            tables
                .get("claims")
                .unwrap()
                .column("procedure")
                .unwrap()
                .get(0),
            Some(Cell::Binary(b"HCPC"))
        );
    }

    #[test]
    fn an_element_defined_without_components_is_read_as_one_text() {
        let spec = spec();
        let (tables, diagnostics) = project(&spec, "HD*A:B~TR~");
        assert_eq!(rendered(&diagnostics), Vec::<String>::new());
        assert_eq!(
            tables.get("heads").unwrap().column("batch").unwrap().get(0),
            Some(Cell::Binary(b"A:B"))
        );
        let (_, diagnostics) = project(&spec, "HD*A:BCDE~TR~");
        assert_eq!(
            rendered(&diagnostics),
            vec![
                "SNIP 2 · element HD01 (batch) has length 6; the spec allows 1 to 5 · segment #0, element 1 · at head#1 · datum \"A:BCDE\""
            ]
        );
    }

    #[test]
    fn only_captured_segments_with_a_definition_are_checked() {
        let spec = spec();
        let (_, diagnostics) = project(&spec, "HD*B1~ZZ*whatever~QQ*!~CL*C1*1~RF~TR~");
        assert_eq!(rendered(&diagnostics), Vec::<String>::new());

        // `AJ` is defined, but no open loop holds it here: it is unmatched,
        // so its empty required `AJ01` is not reported and it gives no row.
        let (tables, diagnostics) = project(&spec, "HD*B1~AJ~TR~");
        assert_eq!(rendered(&diagnostics), Vec::<String>::new());
        assert_eq!(tables.get("adjustments").unwrap().len(), 0);

        // `CL04` is read by two columns; its one bad component is reported once.
        let (tables, diagnostics) = project(&spec, "HD*B1~CL*C1*1**HC:TOOLONG~TR~");
        assert_eq!(
            rendered(&diagnostics),
            vec![
                "SNIP 2 · element CL04-2 (code) has length 7; the spec allows at most 5 · segment #1, element 4, component 2 · at head#1/claim#1 · datum \"TOOLONG\""
            ]
        );
        let claims = tables.get("claims").unwrap();
        assert_eq!(
            claims.column("procedure").unwrap().get(0),
            Some(Cell::Binary(b"HC:TOOLONG"))
        );
        assert_eq!(
            claims.column("code").unwrap().get(0),
            Some(Cell::Binary(b"TOOLONG"))
        );
    }

    #[test]
    fn the_spec_tables_give_the_columns_and_their_types() {
        let spec = spec();
        let (tables, _) = project(&spec, "");
        let kinds: Vec<(&str, ColumnType)> = tables
            .get("lines")
            .unwrap()
            .columns()
            .iter()
            .map(|(name, column)| (name.as_str(), column.kind()))
            .collect();
        assert_eq!(
            kinds,
            vec![
                ("row", ColumnType::Int64 { scale: 0 }),
                ("segment", ColumnType::Int64 { scale: 0 }),
                ("head", ColumnType::Int64 { scale: 0 }),
                ("claim", ColumnType::Int64 { scale: 0 }),
                ("code", ColumnType::Binary),
                ("date", ColumnType::Date32),
                ("time", ColumnType::Time32),
            ]
        );
        let names: Vec<&str> = tables.iter().map(Table::name).collect();
        assert_eq!(names, vec!["adjustments", "claims", "heads", "lines"]);
    }

    /// An `R` value written the way X12 writes it, with two decimals.
    fn money(cents: i64) -> String {
        let sign = if cents < 0 { "-" } else { "" };
        let cents = cents.unsigned_abs();
        format!("{sign}{}.{:02}", cents / 100, cents % 100)
    }

    proptest! {
        #[test]
        fn valid_values_never_raise_a_diagnostic(
            claim_id in "[A-Z0-9]{1,10}",
            cents in -99_999_999i64..99_999_999,
            units in 0i64..99,
            (year, month, day) in (1900i32..2100, 1i32..=12, 1i32..=28),
            seconds in 0i32..86_400,
        ) {
            let spec = spec();
            let input = format!(
                "HD*B1~CL*{claim_id}*{}*{units}*HC:X1~LN*L1~DT*472*{year:04}{month:02}{day:02}*{:02}{:02}{:02}~TR~",
                money(cents),
                seconds / 3600,
                seconds / 60 % 60,
                seconds % 60,
            );
            let (tables, diagnostics) = project(&spec, &input);
            prop_assert_eq!(rendered(&diagnostics), Vec::<String>::new());
            let claims = tables.get("claims").unwrap();
            prop_assert_eq!(claims.column("charge").unwrap().get(0), Some(Cell::Decimal128(i128::from(cents))));
            prop_assert_eq!(claims.column("units").unwrap().get(0), Some(Cell::Int64(units)));
            let lines = tables.get("lines").unwrap();
            prop_assert_eq!(lines.column("time").unwrap().get(0), Some(Cell::Time32(seconds)));
        }

        #[test]
        fn random_elements_never_panic_and_rows_stay_aligned(
            bodies in proptest::collection::vec("[A-Z0-9*:.\\-]{0,24}", 0..8),
        ) {
            let spec = spec();
            let mut input = String::from("HD*B1~");
            for (i, body) in bodies.iter().enumerate() {
                let id = ["CL", "LN", "DT", "AJ", "RF", "NM"][i % 6];
                input.push_str(&format!("{id}*{body}~"));
            }
            input.push_str("TR~");
            let (tables, _) = project(&spec, &input);
            for table in &tables {
                for (_, column) in table.columns() {
                    prop_assert_eq!(column.len(), table.len());
                }
            }
        }
    }
}
