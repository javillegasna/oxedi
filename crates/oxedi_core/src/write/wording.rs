//! The pieces the writer's messages are written from: where a column or an
//! anchor reads, what writes a loop, and the occurrences a selector matches.

use std::fmt;

use crate::message::render_names;

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
