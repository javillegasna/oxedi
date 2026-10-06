//! The bridge from Arrow tables to the core's columns, for writing.
//!
//! A table comes in through the Arrow PyCapsule stream interface (pyarrow,
//! Polars and the tables of a parse export it); an object without it, such
//! as a pandas frame, is read through `pyarrow.table`. Each column is
//! converted to the type the spec gives it: text and bytes of any Arrow
//! layout to bytes, integers (and whole floats) to integers, decimals
//! rescaled exactly, dates and times from their Arrow units. A value that
//! would change on the way is refused with the table, column and row.

use arrow_array::cast::AsArray;
use arrow_array::ffi_stream::{ArrowArrayStreamReader, FFI_ArrowArrayStream};
use arrow_array::types::{
    Date32Type, Date64Type, Decimal128Type, Float32Type, Float64Type, Int8Type, Int16Type,
    Int32Type, Int64Type, Time32MillisecondType, Time32SecondType, Time64MicrosecondType,
    Time64NanosecondType, UInt8Type, UInt16Type, UInt32Type, UInt64Type,
};
use arrow_array::{Array, RecordBatch, RecordBatchReader};
use arrow_schema::{DataType, TimeUnit};
use oxedi_core::project::table_columns;
use oxedi_core::{Cell, ColumnType, Spec, Table, Tables};
use pyo3::prelude::*;
use pyo3::types::{PyCapsule, PyCapsuleMethods, PyMapping};

use crate::tables::extra;

/// What went wrong reading one table, as the message says it.
pub type Refused = String;

/// The tables of a mapping of table names to Arrow tables, each converted
/// to the spec's schema.
pub fn tables(spec: &Spec, mapping: &Bound<'_, PyMapping>) -> PyResult<Result<Tables, Refused>> {
    let mut out = Vec::new();
    for item in mapping.items()?.iter() {
        let (name, value): (String, Bound<'_, PyAny>) = item.extract()?;
        let Some(def) = spec.table(&name) else {
            let names: Vec<String> = spec
                .tables()
                .iter()
                .map(|def| format!("{:?}", def.name))
                .collect();
            return Ok(Err(format!(
                "table {name:?} is not a table of the spec, whose tables are {}",
                names.join(", ")
            )));
        };
        let schema = table_columns(spec, def);
        let reader = stream(&value)?;
        match table(&name, &schema, reader) {
            Ok(table) => out.push(table),
            Err(refused) => return Ok(Err(refused)),
        }
    }
    Ok(Ok(Tables::new(out)))
}

/// The Arrow stream of a table: its own capsule, or pyarrow's reading of it.
fn stream(value: &Bound<'_, PyAny>) -> PyResult<ArrowArrayStreamReader> {
    let exporter = if value.hasattr("__arrow_c_stream__")? {
        value.clone()
    } else {
        extra(value.py(), "oxedi", "write", "pyarrow", "pandas")?
            .getattr("table")?
            .call1((value,))?
    };
    let capsule = exporter.call_method0("__arrow_c_stream__")?;
    let capsule = capsule.cast::<PyCapsule>()?;
    let pointer = capsule.pointer_checked(Some(c"arrow_array_stream"))?;
    // SAFETY: a capsule named "arrow_array_stream" holds an
    // `ArrowArrayStream` (the Arrow PyCapsule interface). `from_raw` moves
    // the stream out and leaves a released one in its place, so the
    // capsule's destructor releases nothing twice.
    let stream = unsafe { FFI_ArrowArrayStream::from_raw(pointer.as_ptr().cast()) };
    ArrowArrayStreamReader::try_new(stream)
        .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))
}

/// One table, converted batch by batch.
fn table(
    name: &str,
    schema: &[(String, ColumnType)],
    reader: ArrowArrayStreamReader,
) -> Result<Table, Refused> {
    let fields = reader.schema();
    let mut columns = Vec::with_capacity(fields.fields().len());
    for field in fields.fields() {
        let Some((_, kind)) = schema.iter().find(|(column, _)| column == field.name()) else {
            let names: Vec<String> = schema
                .iter()
                .map(|(column, _)| format!("{column:?}"))
                .collect();
            return Err(format!(
                "table {name:?} has no column {:?} in the spec; its columns are {}",
                field.name(),
                names.join(", ")
            ));
        };
        columns.push((field.name().clone(), *kind));
    }
    let mut table = Table::new(name, columns.clone());
    let mut offset = 0;
    for batch in reader {
        let batch = batch.map_err(|err| format!("table {name:?} could not be read: {err}"))?;
        rows(name, &columns, &batch, offset, &mut table)?;
        offset += batch.num_rows();
    }
    Ok(table)
}

/// Appends the rows of one batch.
fn rows(
    name: &str,
    columns: &[(String, ColumnType)],
    batch: &RecordBatch,
    offset: usize,
    table: &mut Table,
) -> Result<(), Refused> {
    let arrays = batch.columns();
    let mut cells = Vec::with_capacity(columns.len());
    for row in 0..batch.num_rows() {
        cells.clear();
        for ((column, kind), array) in columns.iter().zip(arrays) {
            let cell = cell(array.as_ref(), row, *kind).map_err(|reason| {
                format!(
                    "table {name:?} column {column:?} row {}: {reason}",
                    offset + row
                )
            })?;
            cells.push(cell);
        }
        table.push_row(&cells).map_err(|err| err.to_string())?;
    }
    Ok(())
}

/// The value of one row as a cell of the column's type.
fn cell(array: &dyn Array, row: usize, kind: ColumnType) -> Result<Cell<'_>, String> {
    if array.is_null(row) || *array.data_type() == DataType::Null {
        return Ok(Cell::Null);
    }
    let unsupported = || {
        format!(
            "an Arrow {} column cannot hold the spec's {kind} values",
            array.data_type()
        )
    };
    match kind {
        ColumnType::Binary => bytes(array, row).ok_or_else(unsupported).map(Cell::Binary),
        ColumnType::Int64 { .. } => integer(array, row)
            .ok_or_else(unsupported)?
            .map(Cell::Int64),
        ColumnType::Decimal128 { scale, .. } => decimal(array, row, scale)
            .ok_or_else(unsupported)?
            .map(Cell::Decimal128),
        ColumnType::Date32 => date(array, row).ok_or_else(unsupported)?.map(Cell::Date32),
        ColumnType::Time32 => time(array, row).ok_or_else(unsupported)?.map(Cell::Time32),
    }
}

fn bytes(array: &dyn Array, row: usize) -> Option<&[u8]> {
    Some(match array.data_type() {
        DataType::Binary => array.as_binary_opt::<i32>()?.value(row),
        DataType::LargeBinary => array.as_binary_opt::<i64>()?.value(row),
        DataType::BinaryView => array.as_binary_view_opt()?.value(row),
        DataType::Utf8 => array.as_string_opt::<i32>()?.value(row).as_bytes(),
        DataType::LargeUtf8 => array.as_string_opt::<i64>()?.value(row).as_bytes(),
        DataType::Utf8View => array.as_string_view_opt()?.value(row).as_bytes(),
        _ => return None,
    })
}

/// An integer of any width, or a float with no fraction; `None` for another
/// type, the reason for a value that does not fit.
fn integer(array: &dyn Array, row: usize) -> Option<Result<i64, String>> {
    let wide = |value: i128| {
        i64::try_from(value).map_err(|_| format!("{value} does not fit a 64-bit integer"))
    };
    let float = |value: f64| {
        if value.is_finite() && value.fract() == 0.0 && value.abs() < 9.2e18 {
            Ok(value as i64)
        } else {
            Err(format!("{value} is not a whole number"))
        }
    };
    Some(match array.data_type() {
        DataType::Int8 => Ok(i64::from(array.as_primitive_opt::<Int8Type>()?.value(row))),
        DataType::Int16 => Ok(i64::from(array.as_primitive_opt::<Int16Type>()?.value(row))),
        DataType::Int32 => Ok(i64::from(array.as_primitive_opt::<Int32Type>()?.value(row))),
        DataType::Int64 => Ok(array.as_primitive_opt::<Int64Type>()?.value(row)),
        DataType::UInt8 => Ok(i64::from(array.as_primitive_opt::<UInt8Type>()?.value(row))),
        DataType::UInt16 => Ok(i64::from(
            array.as_primitive_opt::<UInt16Type>()?.value(row),
        )),
        DataType::UInt32 => Ok(i64::from(
            array.as_primitive_opt::<UInt32Type>()?.value(row),
        )),
        DataType::UInt64 => wide(i128::from(
            array.as_primitive_opt::<UInt64Type>()?.value(row),
        )),
        DataType::Float32 => float(f64::from(
            array.as_primitive_opt::<Float32Type>()?.value(row),
        )),
        DataType::Float64 => float(array.as_primitive_opt::<Float64Type>()?.value(row)),
        _ => return None,
    })
}

/// A decimal rescaled to `scale`, or an integer scaled up; `None` for
/// another type, the reason for a value that does not rescale exactly.
fn decimal(array: &dyn Array, row: usize, scale: u8) -> Option<Result<i128, String>> {
    let (value, from) = match array.data_type() {
        DataType::Decimal128(_, from) => (
            array.as_primitive_opt::<Decimal128Type>()?.value(row),
            i32::from(*from),
        ),
        _ => match integer(array, row)? {
            Ok(value) => (i128::from(value), 0),
            Err(reason) => return Some(Err(reason)),
        },
    };
    let shift = i32::from(scale) - from;
    let factor = 10i128.checked_pow(shift.unsigned_abs());
    Some(match factor {
        Some(factor) if shift >= 0 => value
            .checked_mul(factor)
            .ok_or_else(|| format!("{value} at scale {from} overflows scale {scale}")),
        Some(factor) if value % factor == 0 => Ok(value / factor),
        _ => Err(format!(
            "{value} at scale {from} has more decimals than the column's scale {scale}"
        )),
    })
}

/// Days since 1970-01-01 from a date; `None` for another type.
fn date(array: &dyn Array, row: usize) -> Option<Result<i32, String>> {
    const DAY: i64 = 86_400_000;
    Some(match array.data_type() {
        DataType::Date32 => Ok(array.as_primitive_opt::<Date32Type>()?.value(row)),
        DataType::Date64 => {
            let millis = array.as_primitive_opt::<Date64Type>()?.value(row);
            if millis % DAY == 0 {
                i32::try_from(millis / DAY).map_err(|_| format!("{millis} ms is out of range"))
            } else {
                Err(format!("{millis} ms is not a whole day"))
            }
        }
        _ => return None,
    })
}

/// Seconds since midnight from a time; `None` for another type.
fn time(array: &dyn Array, row: usize) -> Option<Result<i32, String>> {
    let (value, per_second) = match array.data_type() {
        DataType::Time32(TimeUnit::Second) => (
            i64::from(array.as_primitive_opt::<Time32SecondType>()?.value(row)),
            1,
        ),
        DataType::Time32(TimeUnit::Millisecond) => (
            i64::from(
                array
                    .as_primitive_opt::<Time32MillisecondType>()?
                    .value(row),
            ),
            1_000,
        ),
        DataType::Time64(TimeUnit::Microsecond) => (
            array
                .as_primitive_opt::<Time64MicrosecondType>()?
                .value(row),
            1_000_000,
        ),
        DataType::Time64(TimeUnit::Nanosecond) => (
            array.as_primitive_opt::<Time64NanosecondType>()?.value(row),
            1_000_000_000,
        ),
        _ => return None,
    };
    Some(if value % per_second == 0 {
        i32::try_from(value / per_second).map_err(|_| format!("{value} is out of range"))
    } else {
        Err(format!("{value} is not a whole second"))
    })
}
