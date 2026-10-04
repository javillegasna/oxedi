//! A C interface over `edi835_core`: parse a buffer, pick one projected table,
//! and read its columns through the core's own Arrow-layout buffers.
//!
//! Every pointer handed out stays valid until `oxedi835_table_free`.

use std::ffi::{CStr, CString, c_char};
use std::ptr;

use edi835_core::{Column, Document, Processor, Spec, Tables};

/// One projected table and the NUL-terminated names of its columns.
pub struct Oxedi835Table {
    tables: Tables,
    index: usize,
    names: Vec<CString>,
}

/// The buffers of one column, laid out as Apache Arrow lays them out.
#[repr(C)]
pub struct Oxedi835Column {
    /// 0 binary, 1 int64, 2 decimal128, 3 date32, 4 time32 (seconds).
    pub kind: u32,
    pub precision: u8,
    pub scale: u8,
    /// Validity bitmap, least significant bit first; null when every row is valid.
    pub validity: *const u8,
    /// Binary only: rows + 1 offsets into `data`.
    pub offsets: *const i32,
    /// Binary bytes, or the fixed-width values of the other kinds.
    pub data: *const u8,
}

fn error_out(error: *mut *mut c_char, message: String) {
    if !error.is_null() {
        let text = CString::new(message).unwrap_or_default();
        unsafe { *error = text.into_raw() };
    }
}

/// Parses `len` bytes at `bytes` with the built-in 835 spec and keeps the
/// table named `table`. On failure returns null and sets `*error`, which the
/// caller frees with `oxedi835_string_free`.
///
/// # Safety
///
/// `bytes` points at `len` readable bytes; `table` is a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn oxedi835_parse(
    bytes: *const u8,
    len: usize,
    table: *const c_char,
    error: *mut *mut c_char,
) -> *mut Oxedi835Table {
    if bytes.is_null() || table.is_null() {
        error_out(error, "oxedi835_parse: null argument".to_owned());
        return ptr::null_mut();
    }
    let input = unsafe { std::slice::from_raw_parts(bytes, len) };
    let wanted = unsafe { CStr::from_ptr(table) }.to_string_lossy();
    let document = match Document::parse(input) {
        Ok(document) => document,
        Err(err) => {
            error_out(error, err.to_string());
            return ptr::null_mut();
        }
    };
    let spec = Spec::builtin_835();
    let (tables, _diagnostics) = Processor::run(&spec, &document);
    let Some(index) = tables.iter().position(|t| t.name() == wanted) else {
        let names: Vec<&str> = tables.iter().map(|t| t.name()).collect();
        error_out(
            error,
            format!("no table {wanted:?}; the spec projects {names:?}"),
        );
        return ptr::null_mut();
    };
    let names = tables
        .iter()
        .nth(index)
        .map(|t| {
            t.columns()
                .iter()
                .map(|(name, _)| CString::new(name.as_str()).unwrap_or_default())
                .collect()
        })
        .unwrap_or_default();
    Box::into_raw(Box::new(Oxedi835Table {
        tables,
        index,
        names,
    }))
}

fn table(handle: *const Oxedi835Table) -> Option<&'static edi835_core::Table> {
    let handle = unsafe { handle.as_ref() }?;
    handle.tables.iter().nth(handle.index)
}

/// Number of rows of the table.
///
/// # Safety
///
/// `handle` comes from `oxedi835_parse`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn oxedi835_table_rows(handle: *const Oxedi835Table) -> usize {
    table(handle).map_or(0, |t| t.len())
}

/// Number of columns of the table.
///
/// # Safety
///
/// `handle` comes from `oxedi835_parse`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn oxedi835_table_columns(handle: *const Oxedi835Table) -> usize {
    table(handle).map_or(0, |t| t.columns().len())
}

/// Name of column `column`, or null past the last column.
///
/// # Safety
///
/// `handle` comes from `oxedi835_parse`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn oxedi835_table_column_name(
    handle: *const Oxedi835Table,
    column: usize,
) -> *const c_char {
    unsafe { handle.as_ref() }
        .and_then(|h| h.names.get(column))
        .map_or(ptr::null(), |name| name.as_ptr())
}

/// Fills `*out` with the buffers of column `column`; false past the last column.
///
/// # Safety
///
/// `handle` comes from `oxedi835_parse`; `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn oxedi835_table_column(
    handle: *const Oxedi835Table,
    column: usize,
    out: *mut Oxedi835Column,
) -> bool {
    let Some((_, data)) = table(handle).and_then(|t| t.columns().get(column)) else {
        return false;
    };
    let validity = if data.null_count() > 0 {
        data.validity().as_bytes().as_ptr()
    } else {
        ptr::null()
    };
    let (kind, precision, scale, offsets, bytes) = match data.column() {
        Column::Binary { offsets, data } => (0, 0, 0, offsets.as_ptr(), data.as_ptr()),
        Column::Int64 { values, scale } => (1, 0, *scale, ptr::null(), values.as_ptr().cast()),
        Column::Decimal128 {
            values,
            precision,
            scale,
        } => (2, *precision, *scale, ptr::null(), values.as_ptr().cast()),
        Column::Date32(values) => (3, 0, 0, ptr::null(), values.as_ptr().cast()),
        Column::Time32(values) => (4, 0, 0, ptr::null(), values.as_ptr().cast()),
    };
    if out.is_null() {
        return false;
    }
    unsafe {
        *out = Oxedi835Column {
            kind,
            precision,
            scale,
            validity,
            offsets,
            data: bytes,
        };
    }
    true
}

/// Frees a table from `oxedi835_parse`.
///
/// # Safety
///
/// `handle` comes from `oxedi835_parse` and is not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn oxedi835_table_free(handle: *mut Oxedi835Table) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle) });
    }
}

/// Frees an error string.
///
/// # Safety
///
/// `text` comes from this library and is not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn oxedi835_string_free(text: *mut c_char) {
    if !text.is_null() {
        drop(unsafe { CString::from_raw(text) });
    }
}
