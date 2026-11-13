//! The pieces the writer's messages are written from: [`Source`] (where a
//! column or an anchor reads), [`WrittenBy`] (what writes an occurrence),
//! [`Matches`] (the occurrences a selector matches), [`Reference`] (a row
//! reference), [`ReadBackOrigin`] (what a read-back diagnostic is about),
//! [`Counted`] (a count and its noun) and [`Numbered`] (numbered lines).

use std::fmt;

use crate::message::render_names;

use super::finding::ValueOrigin;

/// Where a table reads, as the table and its column, or its anchor segment
/// when the column is `None`.
pub(super) struct Source<'a>(pub(super) &'a str, pub(super) Option<&'a str>);

impl fmt::Display for Source<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.1 {
            Some(column) => write!(f, "table {:?} column {column:?}", self.0),
            None => write!(f, "table {:?} anchor segment", self.0),
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
        match self.0 {
            [] => f.write_str("matches none of its occurrences"),
            names => write!(
                f,
                "matches {} of its occurrences ({}); name one with \"occurrence\"",
                names.len(),
                render_names(names)
            ),
        }
    }
}

/// A row reference of a column: the row it names in the parent table, or
/// that it is null.
pub(super) struct Reference<'a>(pub(super) Option<i64>, pub(super) &'a str);

impl fmt::Display for Reference<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(value) => write!(f, "refers to row {value}, which table {:?} lacks", self.1),
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
pub(super) struct Counted(
    pub(super) usize,
    pub(super) &'static str,
    pub(super) &'static str,
);

impl fmt::Display for Counted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let noun = if self.0 == 1 { self.1 } else { self.2 };
        write!(f, "{} {noun}", self.0)
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
