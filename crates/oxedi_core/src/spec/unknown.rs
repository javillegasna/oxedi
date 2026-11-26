//! The errors for a table or a column the spec does not define.

/// A table name that the spec does not define.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownTable {
    /// The name as the caller wrote it.
    pub name: String,
    /// The names of the tables the spec defines, in spec order.
    pub tables: Vec<String>,
}

impl std::error::Error for UnknownTable {}

/// A column that a table of the spec does not have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownColumn {
    /// The table.
    pub table: String,
    /// The column as the caller wrote it.
    pub column: String,
    /// The table's columns, in schema order.
    pub columns: Vec<String>,
}

impl std::error::Error for UnknownColumn {}
