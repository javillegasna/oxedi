//! The parts of the segment being written: a cell's text, checked for
//! delimiters and for a form its element can hold, a fixed text, and the
//! finished segment with where it came from.

use crate::column::Cell;

use super::emit::Emitter;
use super::finding::{Finding, Origin};
use super::refusal::place;
use super::render::{Part, cell_text, write_segment};
use super::trace::Src;

impl Emitter<'_> {
    /// Adds the text of one cell, checked, as an element or component.
    pub(super) fn push_cell(
        &mut self,
        segment: &[u8],
        element: usize,
        component: Option<usize>,
        table: usize,
        column: usize,
        row: usize,
    ) {
        self.traces
            .entry(element, component, Src::Cell { table, row, column });
        let Some(data) = self.data.get(table).and_then(|data| data.column(column)) else {
            self.parts.push(Part {
                element,
                component,
                text: None,
            });
            return;
        };
        let cell = data.get(row).unwrap_or(Cell::Null);
        let spec = self.spec;
        let max = || {
            spec.element_def(segment, element, component)
                .and_then(|def| def.max)
        };
        let start = self.texts.len();
        let written = cell_text(cell, data.kind(), max, &mut self.texts);
        let origin = || Origin::Cell {
            table: self
                .data
                .get(table)
                .map(|data| data.name.clone())
                .unwrap_or_default(),
            row,
            column: self
                .spec
                .tables()
                .get(table)
                .and_then(|def| def.columns.get(column))
                .map(|(name, _)| name.clone())
                .unwrap_or_default(),
        };
        let text = match written {
            Ok(true) => Some(start..self.texts.len()),
            Ok(false) => None,
            Err(reason) => {
                self.texts.truncate(start);
                self.findings.push(Finding::NotWritable {
                    origin: origin(),
                    place: place(segment, element, component),
                    value: data.render(row).unwrap_or_default(),
                    reason,
                });
                None
            }
        };
        if let Some(range) = &text {
            let held = self.texts.get(range.clone()).unwrap_or_default();
            if self.delimiters.iter().any(|(byte, _)| held.contains(byte)) {
                let bytes = held.to_vec();
                self.check_delimiters(&origin(), segment, element, component, &bytes);
            }
        }
        self.parts.push(Part {
            element,
            component,
            text,
        });
    }

    pub(super) fn push_text(&mut self, element: usize, component: Option<usize>, bytes: &[u8]) {
        let start = self.texts.len();
        self.texts.extend_from_slice(bytes);
        self.parts.push(Part {
            element,
            component,
            text: Some(start..self.texts.len()),
        });
    }

    /// Reports a value that holds a delimiter.
    pub(super) fn check_delimiters(
        &mut self,
        origin: &Origin,
        segment: &[u8],
        element: usize,
        component: Option<usize>,
        bytes: &[u8],
    ) {
        for &(delimiter, role) in &self.delimiters {
            if bytes.contains(&delimiter) {
                self.findings.push(Finding::DelimiterInValue {
                    origin: origin.clone(),
                    place: place(segment, element, component),
                    value: bytes.to_vec(),
                    delimiter,
                    role,
                });
            }
        }
    }

    /// Writes the segment built in `parts`, recording the row it comes from.
    pub(super) fn finish_segment(&mut self, id: &[u8], row: Option<(usize, usize)>) {
        write_segment(id, &self.parts, &self.texts, self.separators, &mut self.out);
        self.traces.close(row);
    }
}
