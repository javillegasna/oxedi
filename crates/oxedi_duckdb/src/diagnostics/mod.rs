//! The `diagnostics` table: one row per diagnostic the core reports about a
//! file, with the attributes of the Python `Diagnostic`, and one row per file
//! that could not be parsed when `ignore_errors` is on.

use oxedi_core::diagnostic::LoopPath;
use oxedi_core::problem::{Detail, Place, Problem};
use oxedi_core::{Cell, ColumnType, Diagnostic, Table};

use crate::error::ReadError;
use crate::types::SqlType;

/// The name `table_name` gives the table.
pub const TABLE: &str = "diagnostics";

/// Who reports a file that could not be parsed.
const READER_ORIGIN: &str = "read_835";

/// The kind of the row of a file that could not be parsed.
const UNPARSABLE_KIND: &str = "NotAnInterchange";

/// Each column's name, core type and whether its text is always valid UTF-8
/// (so a VARCHAR even with `binary := true`).
const COLUMNS: [(&str, ColumnType, bool); 10] = [
    ("level", ColumnType::Int64 { scale: 0 }, false),
    ("kind", ColumnType::Binary, true),
    ("rule", ColumnType::Binary, true),
    ("segment", ColumnType::Int64 { scale: 0 }, false),
    ("element", ColumnType::Int64 { scale: 0 }, false),
    ("component", ColumnType::Int64 { scale: 0 }, false),
    ("path", ColumnType::Binary, true),
    ("datum", ColumnType::Binary, false),
    ("origin", ColumnType::Binary, true),
    ("code", ColumnType::Binary, true),
];

/// The table's columns: name and core type.
pub fn columns() -> Vec<(String, ColumnType)> {
    COLUMNS
        .iter()
        .map(|(name, kind, _)| ((*name).to_owned(), *kind))
        .collect()
}

/// The DuckDB type of each column: `datum` follows `binary`, the other text
/// columns are always VARCHAR.
pub fn types(binary: bool) -> Result<Vec<SqlType>, ReadError> {
    COLUMNS
        .iter()
        .map(|&(name, kind, text)| {
            if text {
                Ok(SqlType::Varchar)
            } else {
                SqlType::of(TABLE, name, kind, binary)
            }
        })
        .collect()
}

/// An empty `diagnostics` table.
pub fn empty() -> Table {
    Table::new(TABLE, columns())
}

/// A position as a BIGINT cell; one past `i64` cannot come from a file the
/// core can index, and is NULL if it ever did.
fn position(value: Option<usize>) -> Cell<'static> {
    value
        .and_then(|value| i64::try_from(value).ok())
        .map_or(Cell::Null, Cell::Int64)
}

/// Appends one row; a row the table refuses is an internal error naming the
/// table.
fn push(table: &mut Table, cells: &[Cell<'_>]) -> Result<(), ReadError> {
    table.push_row(cells).map_err(|error| ReadError::Internal {
        message: format!("table {TABLE:?}: {error}"),
    })
}

/// The table of a parsed file's diagnostics, in stream order. A problem that
/// is not a diagnostic of a file cannot come from the reader, and is an
/// internal error naming the table.
pub fn of_findings(diagnostics: &[Diagnostic]) -> Result<Table, ReadError> {
    let mut table = empty();
    for diagnostic in diagnostics {
        let (
            Place::File {
                segment,
                element,
                component,
                path,
            },
            Detail::Diagnostic {
                level,
                rule,
                datum,
                origin,
                code,
            },
        ) = (diagnostic.place(), diagnostic.detail())
        else {
            return Err(ReadError::Internal {
                message: format!(
                    "table {TABLE:?}: the problem {} is not a diagnostic of a file",
                    diagnostic.kind()
                ),
            });
        };
        push(
            &mut table,
            &[
                Cell::Int64(i64::from(level.number())),
                Cell::Binary(diagnostic.kind().as_bytes()),
                Cell::Binary(rule.to_string().as_bytes()),
                position(segment),
                position(element),
                position(component),
                Cell::Binary(LoopPath(path).to_string().as_bytes()),
                Cell::Binary(datum),
                Cell::Binary(origin.as_bytes()),
                code.map_or(Cell::Null, |code| Cell::Binary(code.as_bytes())),
            ],
        )?;
    }
    Ok(table)
}

/// The one-row table of a file that could not be parsed: level 1, kind
/// `NotAnInterchange`, the error's full text (file included) as the rule,
/// no position or path, the offending bytes as the datum, origin
/// `read_835` and no code. The datum is the bytes themselves with `binary`;
/// otherwise it is their `escape_ascii` text, which is always valid UTF-8
/// (the bytes of a binary file are rarely so).
pub fn of_unparsable(error: &ReadError, datum: &[u8], binary: bool) -> Result<Table, ReadError> {
    let mut table = empty();
    let rule = error.to_string();
    let escaped = datum.escape_ascii().to_string();
    let datum = if binary { datum } else { escaped.as_bytes() };
    push(
        &mut table,
        &[
            Cell::Int64(1),
            Cell::Binary(UNPARSABLE_KIND.as_bytes()),
            Cell::Binary(rule.as_bytes()),
            Cell::Null,
            Cell::Null,
            Cell::Null,
            Cell::Binary(b""),
            Cell::Binary(datum),
            Cell::Binary(READER_ORIGIN.as_bytes()),
            Cell::Null,
        ],
    )?;
    Ok(table)
}

#[cfg(test)]
mod tests;
