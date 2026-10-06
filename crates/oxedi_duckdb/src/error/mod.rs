//! The errors `read_835` reports. Each message names the rule that failed,
//! where it failed (the parameter, or the file, table, column and row) and
//! the offending value.

use std::fmt;

use edi835_core::{ColumnType, DocumentError};

/// The name every message starts with, as DuckDB shows it to the user.
pub const FUNCTION: &str = "read_835";

/// Why `read_835` could not bind or scan.
#[derive(Debug)]
pub enum ReadError {
    /// The path argument is neither a string nor a list of strings.
    PathType {
        /// The DuckDB type of the argument.
        found: String,
    },
    /// The path argument, or one entry of the list, is NULL.
    NullPath {
        /// The 1-based position in the list, or `None` for a single path.
        position: Option<usize>,
    },
    /// The list of paths is empty.
    EmptyPathList,
    /// `table` names no table of the spec.
    UnknownTable {
        /// The name given.
        table: String,
        /// The tables the spec projects.
        known: Vec<String>,
    },
    /// `version` names no built-in spec.
    UnknownVersion {
        /// The version given.
        version: String,
        /// The versions of the built-in specs.
        known: Vec<&'static str>,
    },
    /// DuckDB's file system could not open a file.
    Open {
        /// The path as given or expanded.
        file: String,
        /// DuckDB's message.
        message: String,
    },
    /// DuckDB's file system failed while reading a file.
    Read {
        /// The path as given or expanded.
        file: String,
        /// DuckDB's message.
        message: String,
    },
    /// The file is not an X12 interchange the core can index.
    Parse {
        /// The path as given or expanded.
        file: String,
        /// The core's error.
        source: DocumentError,
    },
    /// A text cell is not valid UTF-8, so it cannot be a VARCHAR.
    InvalidUtf8 {
        /// The path as given or expanded.
        file: String,
        /// The table read.
        table: String,
        /// The column of the cell.
        column: String,
        /// The 0-based row of the cell in the file's table.
        row: usize,
        /// The cell's bytes.
        bytes: Vec<u8>,
    },
    /// The spec a file selected projects the table with other columns than
    /// the ones bound.
    SchemaMismatch {
        /// The path as given or expanded.
        file: String,
        /// The table read.
        table: String,
        /// The version of the spec the file selected.
        version: &'static str,
        /// The version of the spec the columns were bound with.
        bound: &'static str,
    },
    /// A column type of the core has no DuckDB type here.
    UnsupportedType {
        /// The table read.
        table: String,
        /// The column.
        column: String,
        /// The core's type.
        kind: ColumnType,
    },
    /// A panic was caught before it could cross into DuckDB.
    Internal {
        /// The panic's message.
        message: String,
    },
}

/// Quotes each name and joins them with commas.
fn quoted<S: AsRef<str>>(names: &[S]) -> String {
    names
        .iter()
        .map(|name| format!("{:?}", name.as_ref()))
        .collect::<Vec<_>>()
        .join(", ")
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::PathType { found } => write!(
                f,
                "{FUNCTION}: the path must be a VARCHAR or a list of VARCHAR; found {found}"
            ),
            ReadError::NullPath { position: None } => {
                write!(f, "{FUNCTION}: the path must not be NULL")
            }
            ReadError::NullPath {
                position: Some(position),
            } => write!(
                f,
                "{FUNCTION}: every path of the list must be a string; path {position} is NULL"
            ),
            ReadError::EmptyPathList => {
                write!(f, "{FUNCTION}: the list of paths must not be empty")
            }
            ReadError::UnknownTable { table, known } => write!(
                f,
                "{FUNCTION}: unknown table {table:?}; table must be one of {}",
                quoted(known)
            ),
            ReadError::UnknownVersion { version, known } => write!(
                f,
                "{FUNCTION}: unknown version {version:?}; version must be one of {}",
                quoted(known)
            ),
            ReadError::Open { file, message } => {
                write!(f, "{FUNCTION}: {file:?} could not be opened: {message}")
            }
            ReadError::Read { file, message } => {
                write!(f, "{FUNCTION}: {file:?} could not be read: {message}")
            }
            ReadError::Parse { file, source } => {
                write!(
                    f,
                    "{FUNCTION}: {file:?} is not an X12 interchange: {source}"
                )
            }
            ReadError::InvalidUtf8 {
                file,
                table,
                column,
                row,
                bytes,
            } => write!(
                f,
                "{FUNCTION}: {file:?}, table {table:?}, column {column:?}, row {row}: \
                 a VARCHAR must be valid UTF-8; found b\"{}\"; \
                 pass binary := true to read text columns as BLOB",
                bytes.escape_ascii()
            ),
            ReadError::SchemaMismatch {
                file,
                table,
                version,
                bound,
            } => write!(
                f,
                "{FUNCTION}: {file:?} declares version {version}, whose table {table:?} has \
                 other columns than version {bound}, which the query was bound with; \
                 pass version := '{version}' to read such files on their own"
            ),
            ReadError::UnsupportedType {
                table,
                column,
                kind,
            } => write!(
                f,
                "{FUNCTION}: table {table:?}, column {column:?}: the core type {kind} has no DuckDB type"
            ),
            ReadError::Internal { message } => {
                write!(f, "{FUNCTION}: internal error: {message}")
            }
        }
    }
}

impl std::error::Error for ReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ReadError::Parse { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
