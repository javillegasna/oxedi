//! The errors the `edi835` copy format reports. Each message names the rule
//! that failed, where it failed (the option, the input column, or the
//! table, column and row) and the offending value.

use std::fmt;

use oxedi_core::ColumnType;
use oxedi_core::column::RowError;
use oxedi_core::write::WriteError;

/// The name every message starts with: the format's name in `COPY`.
pub const FORMAT: &str = "edi835";

/// Why `COPY … (FORMAT edi835)` could not bind, read its rows or write.
#[derive(Debug)]
pub enum CopyError {
    /// An option the format does not know.
    UnknownOption {
        /// The option as given, lower case.
        name: String,
        /// The options the format knows.
        known: &'static [&'static str],
    },
    /// A file option DuckDB passes on that the format cannot honour.
    FileOption {
        /// The option as given, lower case.
        name: String,
        /// Why it does not apply.
        reason: &'static str,
    },
    /// A required option is missing.
    MissingOption {
        /// The option.
        name: &'static str,
        /// Every required option.
        required: &'static [&'static str],
    },
    /// An option was given without a value.
    NoValue {
        /// The option.
        name: &'static str,
    },
    /// An option's value has the wrong type or form.
    OptionValue {
        /// The option.
        name: &'static str,
        /// The value as given, with its DuckDB type.
        value: String,
        /// What the option takes.
        expected: &'static str,
    },
    /// The time has a fraction of a second.
    FractionalTime {
        /// The value as given, with its DuckDB type.
        value: String,
    },
    /// `version` names no built-in spec.
    UnknownVersion {
        /// The version given.
        version: String,
        /// The versions of the built-in specs.
        known: Vec<&'static str>,
    },
    /// A field of `delimiters` the envelope does not have.
    UnknownDelimiter {
        /// The field as given.
        name: String,
        /// The fields `delimiters` takes.
        known: &'static [&'static str],
    },
    /// A delimiter that is not exactly one byte.
    DelimiterLength {
        /// The delimiter's field.
        name: String,
        /// Its bytes.
        bytes: Vec<u8>,
    },
    /// An input column that is not a list of structs.
    NotRows {
        /// The 1-based position of the column in the query.
        column: usize,
        /// The column's DuckDB type.
        found: String,
    },
    /// No table of the spec has every field of an input column's structs.
    NoTable {
        /// The 1-based position of the column in the query.
        column: usize,
        /// The struct's fields.
        fields: Vec<String>,
        /// The spec's tables.
        tables: Vec<String>,
    },
    /// Several tables of the spec have every field of an input column's
    /// structs.
    AmbiguousTable {
        /// The 1-based position of the column in the query.
        column: usize,
        /// The struct's fields.
        fields: Vec<String>,
        /// The tables that have them all.
        tables: Vec<String>,
    },
    /// Two input columns hold the same table.
    DuplicateTable {
        /// The table.
        table: String,
        /// The 1-based position of the first column.
        first: usize,
        /// The 1-based position of the second column.
        second: usize,
    },
    /// A struct field whose DuckDB type cannot hold the column's values.
    FieldType {
        /// The table.
        table: String,
        /// The field, named after the table's column.
        field: String,
        /// The field's DuckDB type.
        found: String,
        /// The spec's type of the column.
        expected: ColumnType,
    },
    /// A float field for a decimal column: a float may not hold the amount
    /// exactly.
    FloatForDecimal {
        /// The table.
        table: String,
        /// The field, named after the table's column.
        field: String,
        /// The field's DuckDB type.
        found: String,
        /// The spec's type of the column.
        expected: ColumnType,
    },
    /// A value that would change on its way into the column's type.
    Value {
        /// The table.
        table: String,
        /// The column.
        column: String,
        /// The 0-based position of the row in its table.
        row: usize,
        /// What would change.
        reason: String,
    },
    /// The core refused a row, e.g. text past what a column addresses.
    Row(RowError),
    /// The core's writer refused the tables or the envelope; nothing was
    /// written.
    Write(WriteError),
    /// DuckDB asked for a second file in one `COPY`, as `PARTITION_BY` and
    /// `PER_THREAD_OUTPUT` do.
    SecondFile {
        /// The path of the second file.
        path: String,
    },
    /// DuckDB's file system failed on the target file.
    Output {
        /// The path DuckDB gave the format.
        path: String,
        /// The step that failed, e.g. `opened for writing`.
        step: &'static str,
        /// DuckDB's message.
        message: String,
    },
    /// Something that should not happen: a panic caught before it could
    /// cross into DuckDB, or a handle or state DuckDB did not provide.
    Internal {
        /// What went wrong.
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

/// Bytes as a Python bytes literal, e.g. `b'**'`.
pub fn bytes_literal(bytes: &[u8]) -> String {
    format!("b'{}'", bytes.escape_ascii())
}

impl fmt::Display for CopyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CopyError::UnknownOption { name, known } => write!(
                f,
                "{FORMAT}: unknown option {name:?}; the options are {}",
                quoted(known)
            ),
            CopyError::FileOption { name, reason } => {
                write!(f, "{FORMAT}: the option {name:?} does not apply: {reason}")
            }
            CopyError::MissingOption { name, required } => write!(
                f,
                "{FORMAT}: the option {name:?} is required; every one of {} must be given",
                quoted(required)
            ),
            CopyError::NoValue { name } => {
                write!(f, "{FORMAT}: the option {name:?} needs a value")
            }
            CopyError::OptionValue {
                name,
                value,
                expected,
            } => write!(
                f,
                "{FORMAT}: the option {name:?} is {value}; it must be {expected}"
            ),
            CopyError::FractionalTime { value } => write!(
                f,
                "{FORMAT}: the option \"time\" is {value}, which has a fraction of a second; \
                 the envelope holds whole seconds"
            ),
            CopyError::UnknownVersion { version, known } => write!(
                f,
                "{FORMAT}: unknown version {version:?}; version must be one of {}",
                quoted(known)
            ),
            CopyError::UnknownDelimiter { name, known } => write!(
                f,
                "{FORMAT}: delimiters has no field {name:?}; its fields are {}",
                quoted(known)
            ),
            CopyError::DelimiterLength { name, bytes } => write!(
                f,
                "{FORMAT}: delimiter {name} must be exactly one byte, got {} bytes: {}",
                bytes.len(),
                bytes_literal(bytes)
            ),
            CopyError::NotRows { column, found } => write!(
                f,
                "{FORMAT}: input column {column} is {found}; each input column must be a list of \
                 structs holding one table's rows, such as (SELECT list(c) FROM claims c)"
            ),
            CopyError::NoTable {
                column,
                fields,
                tables,
            } => write!(
                f,
                "{FORMAT}: input column {column} holds structs with the fields {}, which no table \
                 of the spec has together; the spec's tables are {}",
                quoted(fields),
                quoted(tables)
            ),
            CopyError::AmbiguousTable {
                column,
                fields,
                tables,
            } => write!(
                f,
                "{FORMAT}: input column {column} holds structs with the fields {}, which the \
                 tables {} all have; add the fields that tell them apart, such as every column \
                 of the table",
                quoted(fields),
                quoted(tables)
            ),
            CopyError::DuplicateTable {
                table,
                first,
                second,
            } => write!(
                f,
                "{FORMAT}: input columns {first} and {second} both hold table {table:?}; give \
                 each table once, with every row in one list"
            ),
            CopyError::FieldType {
                table,
                field,
                found,
                expected,
            } => write!(
                f,
                "{FORMAT}: table {table:?} field {field:?} is {found}; the spec's column is \
                 {expected}, which takes {}",
                accepted(*expected)
            ),
            CopyError::FloatForDecimal {
                table,
                field,
                found,
                expected,
            } => write!(
                f,
                "{FORMAT}: table {table:?} field {field:?} is {found}, which is refused for the \
                 spec's {expected} values: a float may not hold the amount exactly; cast it to \
                 DECIMAL"
            ),
            CopyError::Value {
                table,
                column,
                row,
                reason,
            } => write!(
                f,
                "{FORMAT}: table {table:?} column {column:?} row {row}: {reason}"
            ),
            CopyError::Row(error) => write!(f, "{FORMAT}: {error}"),
            CopyError::Write(error) => write!(f, "{FORMAT}: {error}"),
            CopyError::SecondFile { path } => write!(
                f,
                "{FORMAT}: the COPY asks for a second file, {path:?}, but the format writes all \
                 the rows as one interchange in one file; PARTITION_BY and PER_THREAD_OUTPUT do \
                 not apply"
            ),
            CopyError::Output {
                path,
                step,
                message,
            } => write!(f, "{FORMAT}: {path:?} could not be {step}: {message}"),
            CopyError::Internal { message } => {
                write!(f, "{FORMAT}: internal error: {message}")
            }
        }
    }
}

/// The DuckDB types a column of the spec's type takes.
pub fn accepted(kind: ColumnType) -> &'static str {
    match kind {
        ColumnType::Binary => "VARCHAR or BLOB",
        ColumnType::Int64 { .. } => "an integer type or a FLOAT or DOUBLE holding whole numbers",
        ColumnType::Decimal128 { .. } => "DECIMAL or an integer type",
        ColumnType::Date32 => "DATE, or TIMESTAMP at midnight",
        ColumnType::Time32 => "TIME in whole seconds",
    }
}

impl std::error::Error for CopyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CopyError::Row(source) => Some(source),
            CopyError::Write(source) => Some(source),
            _ => None,
        }
    }
}
