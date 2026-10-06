//! Where each written segment and element came from, so that a diagnostic
//! about the written file names the table, row and column (or envelope
//! field) behind it.

use super::envelope::Field;
use super::finding::Origin;

/// The source of one written element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Src {
    /// A cell: indexes of the table and column, position of the row.
    Cell {
        table: usize,
        row: usize,
        column: usize,
    },
    /// A field of the envelope.
    Field(Field),
}

/// One element's source, in the segment being written.
#[derive(Debug, Clone, Copy)]
struct Entry {
    element: usize,
    component: Option<usize>,
    src: Src,
}

/// The sources of every written segment, in file order.
#[derive(Debug, Clone, Default)]
pub(super) struct Traces {
    entries: Vec<Entry>,
    /// By segment: where its entries start, and the row it comes from.
    segments: Vec<(usize, Option<(usize, usize)>)>,
    /// Where the open segment's entries start.
    open: usize,
}

impl Traces {
    /// The number of segments written.
    pub(super) fn len(&self) -> usize {
        self.segments.len()
    }

    /// Records the source of an element of the open segment.
    pub(super) fn entry(&mut self, element: usize, component: Option<usize>, src: Src) {
        self.entries.push(Entry {
            element,
            component,
            src,
        });
    }

    /// Closes the open segment, written from `row` (`(table, position)`).
    pub(super) fn close(&mut self, row: Option<(usize, usize)>) {
        self.segments.push((self.open, row));
        self.open = self.entries.len();
    }

    /// The source of an element of segment `index`: the cell or field that
    /// wrote it, else the row of the segment. `names` gives a table's name
    /// and a column's name by index.
    pub(super) fn origin(
        &self,
        index: usize,
        element: Option<usize>,
        component: Option<usize>,
        names: &dyn Fn(usize, Option<usize>) -> String,
    ) -> Option<Origin> {
        let &(start, row) = self.segments.get(index)?;
        let end = self
            .segments
            .get(index + 1)
            .map_or(self.entries.len(), |(next, _)| *next);
        let entries = self.entries.get(start..end).unwrap_or_default();
        let found = element.and_then(|element| {
            entries.iter().find(|entry| {
                entry.element == element
                    && (component.is_none()
                        || entry.component.is_none()
                        || entry.component == component)
            })
        });
        match found.map(|entry| entry.src) {
            Some(Src::Cell { table, row, column }) => Some(Origin::Cell {
                table: names(table, None),
                row,
                column: names(table, Some(column)),
            }),
            Some(Src::Field(field)) => Some(Origin::Envelope {
                field: field.name().to_string(),
            }),
            None => row.map(|(table, row)| Origin::Row {
                table: names(table, None),
                row,
            }),
        }
    }
}
