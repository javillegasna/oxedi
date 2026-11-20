//! The error for a table the spec does not define.

use std::fmt;

use crate::message::render_names;

/// A table name that the spec does not define.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownTable {
    /// The name as the caller wrote it.
    pub name: String,
    /// The names of the tables the spec defines, in spec order.
    pub tables: Vec<String>,
}

impl fmt::Display for UnknownTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "table {:?} is not a table of the spec; its tables are {}",
            self.name,
            render_names(&self.tables)
        )
    }
}

impl std::error::Error for UnknownTable {}
