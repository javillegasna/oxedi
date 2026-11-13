//! The pieces the writer's messages are written from: where a column or an
//! anchor reads, what writes a loop, and the occurrences a selector matches.

use std::fmt;

use crate::message::render_names;

use super::finding::ValueOrigin;

/// Where a table reads: a column of it, or its anchor segment when `column`
/// is `None`.
pub(super) struct Source<'a> {
    pub(super) table: &'a str,
    pub(super) column: Option<&'a str>,
}

impl fmt::Display for Source<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.column {
            Some(column) => write!(f, "table {:?} column {column:?}", self.table),
            None => write!(f, "table {:?} anchor segment", self.table),
        }
    }
}

/// The table that writes an occurrence, as a parenthetical clause (with its
/// commas), or that no table does.
pub(super) struct WrittenBy<'a>(pub(super) Option<&'a str>);

impl fmt::Display for WrittenBy<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(table) => write!(f, ", written by table {table:?},"),
            None => f.write_str(", which no table writes,"),
        }
    }
}

/// How many of a loop's occurrences a selector matches, naming them when it
/// matches some.
pub(super) struct Matches<'a>(pub(super) &'a [String]);

impl fmt::Display for Matches<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return f.write_str("matches none of its occurrences");
        }
        write!(
            f,
            "matches {} of its occurrences ({}); name one with \"occurrence\"",
            self.0.len(),
            render_names(self.0)
        )
    }
}

/// A row reference of a column: the row it names in the parent table, or
/// that it is null.
pub(super) struct Reference<'a> {
    pub(super) value: Option<i64>,
    pub(super) parent: &'a str,
}

impl fmt::Display for Reference<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value {
            Some(value) => write!(
                f,
                "refers to row {value}, which table {:?} lacks",
                self.parent
            ),
            None => f.write_str("is null"),
        }
    }
}

/// What a read-back diagnostic is about: the cell, row or field it came
/// from, or the written file as a whole.
pub(super) struct ReadBackOrigin<'a>(pub(super) Option<&'a ValueOrigin>);

impl fmt::Display for ReadBackOrigin<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(origin) => write!(f, "{origin}"),
            None => f.write_str("the written file"),
        }
    }
}

/// A count and its noun, in the singular for one and the plural otherwise.
pub(super) struct Counted {
    pub(super) count: usize,
    pub(super) one: &'static str,
    pub(super) many: &'static str,
}

impl fmt::Display for Counted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let noun = if self.count == 1 { self.one } else { self.many };
        write!(f, "{} {noun}", self.count)
    }
}

/// Items one per line, each after a line break and its 1-based number.
pub(super) struct Numbered<'a, T>(pub(super) &'a [T]);

impl<T: fmt::Display> fmt::Display for Numbered<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, item) in self.0.iter().enumerate() {
            write!(f, "\n{}. {item}", index + 1)?;
        }
        Ok(())
    }
}
