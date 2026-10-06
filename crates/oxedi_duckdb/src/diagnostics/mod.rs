//! The `diagnostics` table: one row per finding of the core about a file,
//! with the attributes of the Python `Diagnostic`, and one row per file that
//! could not be parsed when `ignore_errors` is on.

use oxedi_core::{Cell, ColumnType, Diagnostic, DocumentError, IsaError, Rule, SnipLevel, Table};

use crate::error::ReadError;
use crate::schema::SqlType;

/// The name `table_name` gives the table.
pub const TABLE: &str = "diagnostics";

/// Who reports the parser's own findings, as the Python binding names it.
const PARSER_ORIGIN: &str = "oxedi";

/// Who reports a file that could not be parsed.
const READER_ORIGIN: &str = "read_835";

/// The kind of the row of a file that could not be parsed.
const UNPARSABLE_KIND: &str = "NotAnInterchange";

/// The most bytes of a truncated ISA kept as the datum: an ISA's length.
const ISA_LEN: usize = 106;

/// The bytes a UTF-8 byte order mark takes.
const BOM_LEN: usize = 3;

/// Each column's name, core type and whether its text is always valid
/// UTF-8 (so a VARCHAR even with `binary := true`), in order.
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
        .map(|(name, kind, text)| {
            if *text {
                Ok(SqlType::Varchar)
            } else {
                SqlType::of(TABLE, name, *kind, binary)
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

fn level(level: SnipLevel) -> i64 {
    match level {
        SnipLevel::L1 => 1,
        SnipLevel::L2 => 2,
        SnipLevel::L3 => 3,
    }
}

/// Appends one row; a row the table refuses is an internal error naming the
/// table.
fn push(table: &mut Table, cells: &[Cell<'_>]) -> Result<(), ReadError> {
    table.push_row(cells).map_err(|error| ReadError::Internal {
        message: format!("table {TABLE:?}: {error}"),
    })
}

/// The table of a parsed file's findings, in stream order.
pub fn of_findings(diagnostics: &[Diagnostic]) -> Result<Table, ReadError> {
    let mut table = empty();
    for diagnostic in diagnostics {
        let (origin, code) = match &diagnostic.rule {
            Rule::External { origin, code, .. } => (origin.as_str(), code.as_deref()),
            _ => (PARSER_ORIGIN, None),
        };
        let rule = diagnostic.rule.to_string();
        let path = diagnostic
            .path
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("/");
        push(
            &mut table,
            &[
                Cell::Int64(level(diagnostic.level)),
                Cell::Binary(diagnostic.rule.kind().as_bytes()),
                Cell::Binary(rule.as_bytes()),
                position(diagnostic.segment),
                position(diagnostic.element),
                position(diagnostic.component),
                Cell::Binary(path.as_bytes()),
                Cell::Binary(&diagnostic.datum),
                Cell::Binary(origin.as_bytes()),
                code.map_or(Cell::Null, |code| Cell::Binary(code.as_bytes())),
            ],
        )?;
    }
    Ok(table)
}

/// The bytes a parse error is about: the bytes found instead of `ISA`, the
/// start of a truncated ISA, or nothing for a file too long to index.
pub fn datum<'a>(error: &'a DocumentError, bytes: &'a [u8]) -> &'a [u8] {
    match error {
        DocumentError::Isa(IsaError::NotIsa { found, .. }) => found,
        DocumentError::Isa(IsaError::Truncated {
            byte_order_mark,
            whitespace,
            ..
        }) => {
            let skipped = if *byte_order_mark { BOM_LEN } else { 0 }.saturating_add(*whitespace);
            let rest = bytes.get(skipped..).unwrap_or_default();
            rest.get(..ISA_LEN).unwrap_or(rest)
        }
        DocumentError::Size(_) => &[],
    }
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
