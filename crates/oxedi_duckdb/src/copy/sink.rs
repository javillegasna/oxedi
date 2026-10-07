//! Appending the rows of one input chunk to the tables being built.
//!
//! The input is one flat STRUCT vector with one child per table. Each child
//! is a LIST vector: per input row a list entry (offset and length) into a
//! child STRUCT vector, whose children hold the fields. A NULL input row, a
//! NULL list or a NULL struct adds no row; a NULL field is a null cell.

use std::ffi::c_void;

use libduckdb_sys as ffi;
use oxedi_core::{Cell, ColumnType, Table};

use super::convert;
use super::error::CopyError;
use super::input::{BoundTable, EnumIndex, FieldType, Integer, Unit};

/// Appends the rows of `chunk` to `tables`, one per bound table.
///
/// # Safety
///
/// `chunk` must be the flattened input chunk DuckDB passed to the running
/// sink callback: one STRUCT column with one field per entry of `bound`,
/// of the types they were bound from.
pub unsafe fn append(
    chunk: ffi::duckdb_data_chunk,
    bound: &[BoundTable],
    tables: &mut [Table],
) -> Result<(), CopyError> {
    // SAFETY: `chunk` is live (caller contract).
    let (rows, count) = unsafe {
        (
            ffi::duckdb_data_chunk_get_size(chunk),
            ffi::duckdb_data_chunk_get_column_count(chunk),
        )
    };
    if count != 1 || tables.len() != bound.len() {
        return Err(CopyError::Internal {
            message: format!(
                "the chunk has {count} columns and the state {} tables for {} bound tables",
                tables.len(),
                bound.len()
            ),
        });
    }
    // SAFETY: the chunk has one column, checked above.
    let input = unsafe { ffi::duckdb_data_chunk_get_vector(chunk, 0) };
    for (index, (fields, table)) in bound.iter().zip(tables.iter_mut()).enumerate() {
        // SAFETY: the STRUCT vector has one child per bound table, in order
        // (caller contract).
        let list = unsafe { ffi::duckdb_struct_vector_get_child(input, index as ffi::idx_t) };
        // SAFETY: the child is the table's flat LIST of STRUCT, and the
        // input STRUCT vector has `rows` rows.
        unsafe { append_table(input, list, rows, fields, table) }?;
    }
    Ok(())
}

/// Whether row `row` of a vector holds a value.
///
/// # Safety
///
/// `vector` must be a live vector of at least `row + 1` rows.
unsafe fn valid(vector: ffi::duckdb_vector, row: u64) -> bool {
    // SAFETY: the vector is live (caller contract); a null mask means every
    // row is valid.
    unsafe {
        let validity = ffi::duckdb_vector_get_validity(vector);
        validity.is_null() || ffi::duckdb_validity_row_is_valid(validity, row)
    }
}

/// # Safety
///
/// `input` must be the live flat STRUCT vector of `rows` rows that holds
/// `list`, a flat LIST vector of STRUCT whose struct fields are of the types
/// `bound` was bound from.
unsafe fn append_table(
    input: ffi::duckdb_vector,
    list: ffi::duckdb_vector,
    rows: ffi::idx_t,
    bound: &BoundTable,
    table: &mut Table,
) -> Result<(), CopyError> {
    // SAFETY: `list` is a live LIST vector (caller contract): its data holds
    // `rows` list entries, its child is the STRUCT vector of `size` rows.
    // The child and the child's own children are read as flat vectors:
    // `CCopyToSink` flattens the chunk before the callback, and DuckDB keeps
    // the child vector of a list flat (flattening a constant list flattens
    // its entries, not its child). The C API cannot tell a vector's kind, so
    // this rests on that DuckDB invariant.
    let (entries, structs, size) = unsafe {
        (
            ffi::duckdb_vector_get_data(list).cast::<ffi::duckdb_list_entry>(),
            ffi::duckdb_list_vector_get_child(list),
            ffi::duckdb_list_vector_get_size(list),
        )
    };
    let fields: Vec<Field<'_>> = bound
        .fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            // SAFETY: the STRUCT vector has one child per bound field, in
            // order (caller contract).
            let vector =
                unsafe { ffi::duckdb_struct_vector_get_child(structs, index as ffi::idx_t) };
            Field {
                // SAFETY: `vector` is a live flat child vector.
                data: unsafe { ffi::duckdb_vector_get_data(vector) },
                vector,
                kind: field.kind,
                input: field.input,
                dictionary: &field.dictionary,
            }
        })
        .collect();
    let mut cells: Vec<Cell<'_>> = Vec::with_capacity(fields.len());
    for row in 0..rows {
        // SAFETY: `row` is below both vectors' row count.
        if !unsafe { valid(input, row) && valid(list, row) } {
            continue;
        }
        // SAFETY: the list data holds `rows` entries and `row` is below it.
        let entry = unsafe { *entries.add(row as usize) };
        let end = entry
            .offset
            .checked_add(entry.length)
            .filter(|end| *end <= size);
        let Some(end) = end else {
            return Err(CopyError::Internal {
                message: format!(
                    "a list entry of table {:?} ends past its {size} child rows",
                    bound.table
                ),
            });
        };
        for at in entry.offset..end {
            // SAFETY: `at` is below the child's size.
            if !unsafe { valid(structs, at) } {
                continue;
            }
            cells.clear();
            for (field, column) in fields.iter().zip(&bound.fields) {
                // SAFETY: `at` is below the child's size, so below each
                // field vector's.
                let cell =
                    unsafe { field.cell(at as usize) }.map_err(|reason| CopyError::Value {
                        table: bound.table.clone(),
                        column: column.name.clone(),
                        row: table.len(),
                        reason,
                    })?;
                cells.push(cell);
            }
            table.push_row(&cells).map_err(CopyError::Row)?;
        }
    }
    Ok(())
}

/// One field vector of the STRUCT being read.
struct Field<'a> {
    vector: ffi::duckdb_vector,
    data: *mut c_void,
    kind: ColumnType,
    input: FieldType,
    /// The values of an `ENUM` field, by index.
    dictionary: &'a [Vec<u8>],
}

impl Field<'_> {
    /// The value at `row` as a cell of the column's type.
    ///
    /// # Safety
    ///
    /// `row` must be below the vector's row count; the vector must be flat
    /// and of the field's input type.
    unsafe fn cell(&self, row: usize) -> Result<Cell<'_>, String> {
        // SAFETY: `row` is below the vector's rows (caller contract).
        if self.input == FieldType::Null || !unsafe { valid(self.vector, row as u64) } {
            return Ok(Cell::Null);
        }
        // SAFETY: the data holds values of the field's physical type
        // (caller contract) and `row` is in range.
        let raw = unsafe { self.raw(row) };
        let mismatch = || {
            format!(
                "an internal mismatch: a {:?} field for a {} column",
                self.input, self.kind
            )
        };
        match (self.kind, raw) {
            (ColumnType::Binary, Raw::Bytes(bytes)) => Ok(Cell::Binary(bytes)),
            (ColumnType::Binary, Raw::Key(key)) => usize::try_from(key)
                .ok()
                .and_then(|index| self.dictionary.get(index))
                .map(|bytes| Cell::Binary(bytes))
                .ok_or_else(|| {
                    format!(
                        "the dictionary key {key} is outside the column's {} dictionary values",
                        self.dictionary.len()
                    )
                }),
            (ColumnType::Int64 { .. }, Raw::Signed(value)) => {
                convert::wide_integer(value).map(Cell::Int64)
            }
            (ColumnType::Int64 { .. }, Raw::Unsigned(value)) => {
                convert::unsigned_integer(value).map(Cell::Int64)
            }
            (ColumnType::Int64 { .. }, Raw::Float(value)) => {
                convert::float_integer(value).map(Cell::Int64)
            }
            (ColumnType::Decimal128 { scale, .. }, Raw::Signed(value)) => {
                let value = convert::wide_integer(value)?;
                convert::rescale(i128::from(value), 0, scale).map(Cell::Decimal128)
            }
            (ColumnType::Decimal128 { scale, .. }, Raw::Unsigned(value)) => {
                let value = convert::unsigned_integer(value)?;
                convert::rescale(i128::from(value), 0, scale).map(Cell::Decimal128)
            }
            (ColumnType::Decimal128 { scale, .. }, Raw::Decimal(value, from)) => {
                convert::rescale(value, from, scale).map(Cell::Decimal128)
            }
            (ColumnType::Date32, Raw::Date(days)) => Ok(Cell::Date32(days)),
            (ColumnType::Date32, Raw::Timestamp(value, unit)) => {
                convert::timestamp_date(value, unit).map(Cell::Date32)
            }
            (ColumnType::Time32, Raw::Time(value, per_second)) => {
                convert::whole_seconds(value, per_second).map(Cell::Time32)
            }
            _ => Err(mismatch()),
        }
    }

    /// The value at `row` as DuckDB stores it.
    ///
    /// # Safety
    ///
    /// `row` must be below the vector's row count; the vector must be flat
    /// and of the field's input type.
    unsafe fn raw(&self, row: usize) -> Raw<'_> {
        /// The `row`-th value of the data, read as `T`.
        ///
        /// # Safety
        ///
        /// `data` must hold more than `row` values of type `T`.
        unsafe fn at<T: Copy>(data: *mut c_void, row: usize) -> T {
            // SAFETY: caller contract.
            unsafe { *data.cast::<T>().add(row) }
        }
        let data = self.data;
        // SAFETY: each arm reads the physical type DuckDB stores for the
        // field's type (caller contract).
        unsafe {
            match self.input {
                FieldType::Varchar | FieldType::Blob => {
                    let string = data.cast::<ffi::duckdb_string_t>().add(row);
                    let len = ffi::duckdb_string_t_length(*string) as usize;
                    let start = ffi::duckdb_string_t_data(string).cast::<u8>();
                    Raw::Bytes(if len == 0 || start.is_null() {
                        &[]
                    } else {
                        std::slice::from_raw_parts(start, len)
                    })
                }
                FieldType::Enum(EnumIndex::U8) => Raw::Key(at::<u8>(data, row).into()),
                FieldType::Enum(EnumIndex::U16) => Raw::Key(at::<u16>(data, row).into()),
                FieldType::Enum(EnumIndex::U32) => Raw::Key(at::<u32>(data, row)),
                FieldType::Integer(Integer::I8) => Raw::Signed(at::<i8>(data, row).into()),
                FieldType::Integer(Integer::I16) => Raw::Signed(at::<i16>(data, row).into()),
                FieldType::Integer(Integer::I32) => Raw::Signed(at::<i32>(data, row).into()),
                FieldType::Integer(Integer::I64) => Raw::Signed(at::<i64>(data, row).into()),
                FieldType::Integer(Integer::I128) => {
                    Raw::Signed(hugeint(at::<ffi::duckdb_hugeint>(data, row)))
                }
                FieldType::Integer(Integer::U8) => Raw::Unsigned(at::<u8>(data, row).into()),
                FieldType::Integer(Integer::U16) => Raw::Unsigned(at::<u16>(data, row).into()),
                FieldType::Integer(Integer::U32) => Raw::Unsigned(at::<u32>(data, row).into()),
                FieldType::Integer(Integer::U64) => Raw::Unsigned(at::<u64>(data, row).into()),
                FieldType::Integer(Integer::U128) => {
                    let value = at::<ffi::duckdb_uhugeint>(data, row);
                    Raw::Unsigned((u128::from(value.upper) << 64) | u128::from(value.lower))
                }
                FieldType::Float => Raw::Float(at::<f32>(data, row).into()),
                FieldType::Double => Raw::Float(at::<f64>(data, row)),
                FieldType::Decimal { width, scale } => Raw::Decimal(
                    match width {
                        0..=4 => at::<i16>(data, row).into(),
                        5..=9 => at::<i32>(data, row).into(),
                        10..=18 => at::<i64>(data, row).into(),
                        _ => hugeint(at::<ffi::duckdb_hugeint>(data, row)),
                    },
                    scale,
                ),
                FieldType::Date => Raw::Date(at::<i32>(data, row)),
                FieldType::Time => Raw::Time(at::<i64>(data, row), 1_000_000),
                FieldType::TimeNs => Raw::Time(at::<i64>(data, row), 1_000_000_000),
                FieldType::Timestamp(unit) => Raw::Timestamp(at::<i64>(data, row), unit),
                FieldType::Null | FieldType::Other => Raw::None,
            }
        }
    }
}

/// A value as DuckDB stores it.
enum Raw<'a> {
    Bytes(&'a [u8]),
    /// An `ENUM` index into the field's dictionary.
    Key(u32),
    Signed(i128),
    Unsigned(u128),
    Float(f64),
    /// A decimal's integer and scale.
    Decimal(i128, u8),
    /// Days since 1970-01-01.
    Date(i32),
    /// A time and its count per second.
    Time(i64, i64),
    Timestamp(i64, Unit),
    None,
}

/// A DuckDB `hugeint` as `i128`.
fn hugeint(value: ffi::duckdb_hugeint) -> i128 {
    (i128::from(value.upper) << 64) | i128::from(value.lower)
}
