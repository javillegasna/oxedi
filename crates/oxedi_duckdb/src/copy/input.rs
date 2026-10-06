//! The query's columns: one `LIST(STRUCT(...))` per table of the spec.
//!
//! The C API of a copy function gives the columns' types but not their
//! names, so each column's table is the one table of the spec that has
//! every field of its structs. Each field must be a column of that table,
//! with a DuckDB type its values convert from.

use libduckdb_sys as ffi;
use oxedi_core::ColumnType;
use oxedi_core::write::WriteError;

use super::error::CopyError;
use crate::builtins::TableSchema;
use crate::value::primitive_name;

/// The DuckDB type of a struct field, as far as conversion cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    /// `NULL`: every value is null.
    Null,
    /// `VARCHAR`.
    Varchar,
    /// `BLOB`.
    Blob,
    /// An integer type.
    Integer(Integer),
    /// `FLOAT`.
    Float,
    /// `DOUBLE`.
    Double,
    /// `DECIMAL(width, scale)`.
    Decimal {
        /// Total digits.
        width: u8,
        /// Decimal places.
        scale: u8,
    },
    /// `DATE`.
    Date,
    /// `TIME`, microseconds.
    Time,
    /// `TIME_NS`, nanoseconds.
    TimeNs,
    /// A `TIMESTAMP` without a time zone, in the unit given.
    Timestamp(Unit),
    /// Any other type.
    Other,
}

/// The integer types, by how DuckDB stores them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Integer {
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
}

/// A timestamp's unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Second,
    Milli,
    Micro,
    Nano,
}

impl FieldType {
    /// Whether a column of the spec's `kind` takes values of this type.
    fn converts_to(self, kind: ColumnType) -> bool {
        matches!(
            (self, kind),
            (FieldType::Null, _)
                | (FieldType::Varchar | FieldType::Blob, ColumnType::Binary)
                | (
                    FieldType::Integer(_) | FieldType::Float | FieldType::Double,
                    ColumnType::Int64 { .. }
                )
                | (
                    FieldType::Integer(_) | FieldType::Decimal { .. },
                    ColumnType::Decimal128 { .. }
                )
                | (
                    FieldType::Date | FieldType::Timestamp(_),
                    ColumnType::Date32
                )
                | (FieldType::Time | FieldType::TimeNs, ColumnType::Time32)
        )
    }
}

/// One field of an input column's structs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputField {
    /// The field's name.
    pub name: String,
    /// Its type.
    pub kind: FieldType,
    /// Its DuckDB type as SQL writes it, for messages.
    pub sql: String,
}

/// One input column as its type describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputColumn {
    /// A list of structs with these fields.
    Rows(Vec<InputField>),
    /// Any other type, as SQL writes it.
    Other(String),
}

/// A field bound to its table's column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundField {
    /// The field's name, the column's.
    pub name: String,
    /// The spec's type of the column.
    pub kind: ColumnType,
    /// The field's DuckDB type.
    pub input: FieldType,
}

/// An input column bound to its table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundColumn {
    /// The table.
    pub table: String,
    /// Each field of the column's structs, in the structs' order.
    pub fields: Vec<BoundField>,
}

/// Binds every input column to a table of `tables`.
pub fn bind(
    tables: &[TableSchema],
    columns: &[InputColumn],
) -> Result<Vec<BoundColumn>, CopyError> {
    let mut bound: Vec<BoundColumn> = Vec::with_capacity(columns.len());
    for (index, column) in columns.iter().enumerate() {
        let position = index + 1;
        let fields = match column {
            InputColumn::Rows(fields) => fields,
            InputColumn::Other(found) => {
                return Err(CopyError::NotRows {
                    column: position,
                    found: found.clone(),
                });
            }
        };
        let table = table_of(tables, position, fields)?;
        if let Some(first) = bound.iter().position(|other| other.table == table.name) {
            return Err(CopyError::DuplicateTable {
                table: table.name.clone(),
                first: first + 1,
                second: position,
            });
        }
        let fields = fields
            .iter()
            .map(|field| bind_field(table, field))
            .collect::<Result<Vec<_>, _>>()?;
        bound.push(BoundColumn {
            table: table.name.clone(),
            fields,
        });
    }
    Ok(bound)
}

fn has(table: &TableSchema, field: &str) -> bool {
    table.columns.iter().any(|(column, _)| column == field)
}

/// The one table that has every field; else the error that says why none
/// does.
fn table_of<'t>(
    tables: &'t [TableSchema],
    position: usize,
    fields: &[InputField],
) -> Result<&'t TableSchema, CopyError> {
    let names = || fields.iter().map(|field| field.name.clone()).collect();
    let matching: Vec<&TableSchema> = tables
        .iter()
        .filter(|table| fields.iter().all(|field| has(table, &field.name)))
        .collect();
    match matching.as_slice() {
        [table] => return Ok(table),
        [] => {}
        several => {
            return Err(CopyError::AmbiguousTable {
                column: position,
                fields: names(),
                tables: several.iter().map(|table| table.name.clone()).collect(),
            });
        }
    }
    // No table has every field: the table that has the most of them, when
    // one does, is the one meant, and the first field it lacks is unknown.
    let overlap = |table: &TableSchema| {
        fields
            .iter()
            .filter(|field| has(table, &field.name))
            .count()
    };
    let most = tables.iter().map(overlap).max().unwrap_or(0);
    let best: Vec<&TableSchema> = tables
        .iter()
        .filter(|table| overlap(table) == most)
        .collect();
    if let ([table], true) = (best.as_slice(), most > 0)
        && let Some(field) = fields.iter().find(|field| !has(table, &field.name))
    {
        return Err(CopyError::Write(WriteError::UnknownColumn {
            table: table.name.clone(),
            column: field.name.clone(),
            columns: table.columns.iter().map(|(name, _)| name.clone()).collect(),
        }));
    }
    Err(CopyError::NoTable {
        column: position,
        fields: names(),
        tables: tables.iter().map(|table| table.name.clone()).collect(),
    })
}

fn bind_field(table: &TableSchema, field: &InputField) -> Result<BoundField, CopyError> {
    let kind = table
        .columns
        .iter()
        .find(|(column, _)| *column == field.name)
        .map(|(_, kind)| *kind)
        .ok_or_else(|| CopyError::Internal {
            message: format!(
                "table {:?} lost the column {:?} it was chosen for",
                table.name, field.name
            ),
        })?;
    if !field.kind.converts_to(kind) {
        let float = matches!(field.kind, FieldType::Float | FieldType::Double);
        let (table, field_name, found) =
            (table.name.clone(), field.name.clone(), field.sql.clone());
        return Err(if float && matches!(kind, ColumnType::Decimal128 { .. }) {
            CopyError::FloatForDecimal {
                table,
                field: field_name,
                found,
                expected: kind,
            }
        } else {
            CopyError::FieldType {
                table,
                field: field_name,
                found,
                expected: kind,
            }
        });
    }
    Ok(BoundField {
        name: field.name.clone(),
        kind,
        input: field.kind,
    })
}

/// A logical type the caller owns, destroyed on drop.
struct Owned(ffi::duckdb_logical_type);

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: the handle was created by DuckDB for the caller and is
        // destroyed once.
        unsafe { ffi::duckdb_destroy_logical_type(&mut self.0) };
    }
}

/// The input column of a logical type.
///
/// # Safety
///
/// `logical_type` must be a live logical type handle.
pub unsafe fn column_of(logical_type: ffi::duckdb_logical_type) -> InputColumn {
    // SAFETY: the type is live (caller contract).
    let id = unsafe { ffi::duckdb_get_type_id(logical_type) };
    if id == ffi::DUCKDB_TYPE_DUCKDB_TYPE_LIST {
        // SAFETY: the type is a live LIST; its child type is owned here.
        let child = Owned(unsafe { ffi::duckdb_list_type_child_type(logical_type) });
        // SAFETY: `child.0` is live.
        if unsafe { ffi::duckdb_get_type_id(child.0) } == ffi::DUCKDB_TYPE_DUCKDB_TYPE_STRUCT {
            // SAFETY: `child.0` is a live STRUCT type.
            return InputColumn::Rows(unsafe { struct_fields(child.0) });
        }
    }
    // SAFETY: the type is live (caller contract).
    InputColumn::Other(unsafe { sql_name(logical_type) })
}

/// The fields of a STRUCT type.
///
/// # Safety
///
/// `logical_type` must be a live STRUCT type.
unsafe fn struct_fields(logical_type: ffi::duckdb_logical_type) -> Vec<InputField> {
    // SAFETY: the type is a live STRUCT (caller contract).
    let count = unsafe { ffi::duckdb_struct_type_child_count(logical_type) };
    (0..count)
        .map(|index| {
            // SAFETY: `index` is below the child count; the name and type
            // returned are owned here and released after use.
            let (name, child) = unsafe {
                (
                    take_text(ffi::duckdb_struct_type_child_name(logical_type, index)),
                    Owned(ffi::duckdb_struct_type_child_type(logical_type, index)),
                )
            };
            // SAFETY: `child.0` is live.
            let (kind, sql) = unsafe { (field_type(child.0), sql_name(child.0)) };
            InputField { name, kind, sql }
        })
        .collect()
}

/// A C string DuckDB allocated for the caller, freed after it is copied.
///
/// # Safety
///
/// `text` must be null or a NUL-terminated string allocated by DuckDB.
unsafe fn take_text(text: *mut std::os::raw::c_char) -> String {
    if text.is_null() {
        return String::new();
    }
    // SAFETY: `text` is a live C string (caller contract), freed once.
    unsafe {
        let owned = std::ffi::CStr::from_ptr(text)
            .to_string_lossy()
            .into_owned();
        ffi::duckdb_free(text.cast());
        owned
    }
}

/// The field type of a logical type.
///
/// # Safety
///
/// `logical_type` must be a live logical type handle.
unsafe fn field_type(logical_type: ffi::duckdb_logical_type) -> FieldType {
    // SAFETY: the type is live (caller contract).
    let id = unsafe { ffi::duckdb_get_type_id(logical_type) };
    match id {
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_SQLNULL => FieldType::Null,
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_VARCHAR => FieldType::Varchar,
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_BLOB => FieldType::Blob,
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_TINYINT => FieldType::Integer(Integer::I8),
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_SMALLINT => FieldType::Integer(Integer::I16),
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_INTEGER => FieldType::Integer(Integer::I32),
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_BIGINT => FieldType::Integer(Integer::I64),
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_HUGEINT => FieldType::Integer(Integer::I128),
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_UTINYINT => FieldType::Integer(Integer::U8),
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_USMALLINT => FieldType::Integer(Integer::U16),
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_UINTEGER => FieldType::Integer(Integer::U32),
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_UBIGINT => FieldType::Integer(Integer::U64),
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_UHUGEINT => FieldType::Integer(Integer::U128),
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_FLOAT => FieldType::Float,
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_DOUBLE => FieldType::Double,
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_DECIMAL => FieldType::Decimal {
            // SAFETY: the type is a live DECIMAL.
            width: unsafe { ffi::duckdb_decimal_width(logical_type) },
            // SAFETY: as above.
            scale: unsafe { ffi::duckdb_decimal_scale(logical_type) },
        },
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_DATE => FieldType::Date,
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIME => FieldType::Time,
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIME_NS => FieldType::TimeNs,
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIMESTAMP_S => FieldType::Timestamp(Unit::Second),
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIMESTAMP_MS => FieldType::Timestamp(Unit::Milli),
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIMESTAMP => FieldType::Timestamp(Unit::Micro),
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIMESTAMP_NS => FieldType::Timestamp(Unit::Nano),
        _ => FieldType::Other,
    }
}

/// A logical type as SQL writes it: `DECIMAL(18,3)`, `INTEGER[]`,
/// `STRUCT(a INTEGER)`, else its primitive name.
///
/// # Safety
///
/// `logical_type` must be a live logical type handle.
unsafe fn sql_name(logical_type: ffi::duckdb_logical_type) -> String {
    // SAFETY: the type is live (caller contract).
    let id = unsafe { ffi::duckdb_get_type_id(logical_type) };
    match id {
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_DECIMAL => {
            // SAFETY: the type is a live DECIMAL.
            let (width, scale) = unsafe {
                (
                    ffi::duckdb_decimal_width(logical_type),
                    ffi::duckdb_decimal_scale(logical_type),
                )
            };
            format!("DECIMAL({width},{scale})")
        }
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_LIST => {
            // SAFETY: the type is a live LIST; the child type is owned here.
            let child = Owned(unsafe { ffi::duckdb_list_type_child_type(logical_type) });
            // SAFETY: `child.0` is live.
            format!("{}[]", unsafe { sql_name(child.0) })
        }
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_STRUCT => {
            // SAFETY: the type is a live STRUCT.
            let fields = unsafe { struct_fields(logical_type) };
            let parts: Vec<String> = fields
                .iter()
                .map(|field| format!("{} {}", field.name, field.sql))
                .collect();
            format!("STRUCT({})", parts.join(", "))
        }
        _ => primitive_name(id).to_owned(),
    }
}
