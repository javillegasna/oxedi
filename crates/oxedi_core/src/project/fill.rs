//! Row filling: columns read from captured segments, the rows appended to the
//! tables, the diagnostic helpers and the text read of an element.

use crate::column::{Cell, CellError, ColumnType, RowError, Table};
use crate::diagnostic::{Diagnostic, LoopRef, Rule};
use crate::element::Element;
use crate::segment::Segment;
use crate::spec::{ColumnSource, LoopId};

use super::check::{Checked, Parsed, parse};
use super::{Projector, Row, Slot};

impl<'s> Projector<'s> {
    /// Fills the open rows whose columns read this segment and have no value yet.
    pub(super) fn fill(
        &mut self,
        watchers: &[(usize, usize)],
        segment: &Segment<'_>,
        joined: &mut Vec<u8>,
    ) {
        let spec = self.spec;
        for &(index, column) in watchers {
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
    pub(super) fn segment_rows(&mut self, id: LoopId, segment: &Segment<'_>, joined: &mut Vec<u8>) {
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
        let dropped = append(&mut state.table, &row, &mut self.cells);
        state.row = row;
        self.report_dropped(dropped, index, segment.index);
    }

    /// Reports each text value an append could not store.
    pub(super) fn report_dropped(&mut self, dropped: Vec<Dropped>, table: usize, segment: usize) {
        for (column, bytes, value) in dropped {
            let rule = Rule::ValueDropped {
                table: self.spec.tables()[table].name.clone(),
                column,
                bytes,
            };
            self.push_diagnostic(rule, segment, None, None, &value);
        }
    }

    pub(super) fn report(
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
pub(super) fn leaf_text<'a>(
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

/// `true` when the segment has the element, or the component, a column
/// reads, even if it is empty. Component 1 of a simple element is the
/// element itself; no other component of it is present.
fn is_present(segment: &Segment<'_>, at: Place) -> bool {
    match (segment.element(at.element), at.component) {
        (None, _) => false,
        (Some(Element::Simple(_)), component) => component.is_none_or(|component| component == 1),
        (Some(Element::Composite(_)), None) => true,
        (Some(Element::Composite(parts)), Some(component)) => {
            component.checked_sub(1).is_some_and(|at| at < parts.len())
        }
    }
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
/// Text is copied into `bytes`; an empty text that is present in the
/// segment is an empty value, not null.
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
        // A text column tells a present but empty value (`""`) from an
        // absent one (null); other types have no empty value.
        Parsed::Null if at.kind == ColumnType::Binary && is_present(segment, at) => {
            Slot::Bytes(bytes.len(), bytes.len())
        }
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

/// A text value an append could not store: its column, the byte total the
/// column would have reached, and the value.
type Dropped = (String, usize, Vec<u8>);

/// Appends a collected row: its number, its anchor segment, the rows above
/// it, then its columns; a column no segment matched is null. A text value
/// that would take its column past what `i32` offsets address is stored as
/// null and returned with its column name, the byte total it would have
/// produced and the value itself, so row numbers stay aligned and nothing
/// disappears unreported.
pub(super) fn append(
    table: &mut Table,
    row: &Row,
    scratch: &mut Vec<Cell<'static>>,
) -> Vec<Dropped> {
    let index = |value: usize| i64::try_from(value).map_or(Cell::Null, Cell::Int64);
    // The scratch vector is empty; collecting an empty iterator out of it
    // gives its allocation back under the row's lifetime when std reuses the
    // allocation in place; otherwise this is an ordinary allocation.
    scratch.clear();
    let mut cells: Vec<Cell<'_>> = std::mem::take(scratch)
        .into_iter()
        .map_while(|_| None)
        .collect();
    cells.reserve(2 + row.parents.len() + row.cells.len());
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
        let Some(cell) = at.and_then(|at| cells.get_mut(at)) else {
            break;
        };
        let value = match std::mem::replace(cell, Cell::Null) {
            Cell::Binary(value) => value.to_vec(),
            _ => Vec::new(),
        };
        dropped.push((column, bytes, value));
    }
    cells.clear();
    *scratch = cells.into_iter().map_while(|_| None).collect();
    dropped
}
