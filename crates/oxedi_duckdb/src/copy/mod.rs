//! The `edi835` copy format: `COPY (query) TO 'file' (FORMAT edi835, …)`
//! writes the query's tables as one 835 interchange with the core's writer.
//!
//! The query returns one `LIST(STRUCT(...))` column per table of the spec,
//! each struct a row with the table's columns (`SELECT list(c) FROM claims
//! c`); several query rows concatenate. The bind reads the options (the
//! envelope and the version) and binds each column to its table; the sink
//! appends every chunk's rows; the finalize writes, and only then opens the
//! target, so a refusal leaves no file behind. Every callback catches
//! panics, so none unwinds into DuckDB.
//!
//! - `mod.rs`: registration and the bind, init, sink and finalize callbacks.
//! - `error.rs`: the errors the format reports.
//! - `options.rs`: the options and the envelope they make.
//! - `read.rs`: the options read from DuckDB's value.
//! - `input.rs`: the query's columns bound to the spec's tables.

mod error;
mod input;
mod options;
mod read;

use std::ffi::CString;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use libduckdb_sys as ffi;
use oxedi_core::write::Envelope;

use crate::builtins::Builtins;
use crate::function::{drop_box, panic_message};
use error::{CopyError, FORMAT};
use input::BoundColumn;
use options::Settings;

/// Registers the `edi835` copy format on `connection`, with `builtins` as
/// the function's extra info.
///
/// # Safety
///
/// `connection` must be a live connection; the C API must be initialized.
pub unsafe fn register(
    connection: ffi::duckdb_connection,
    builtins: Arc<Builtins>,
) -> Result<(), String> {
    let name = CString::new(FORMAT).map_err(|err| err.to_string())?;
    let function = CopyFunction::new();
    // SAFETY: `function` is the live handle created above; DuckDB copies the
    // name, and owns the extra info from here, freeing it with `drop_box`.
    unsafe {
        ffi::duckdb_copy_function_set_name(function.0, name.as_ptr());
        let extra = Box::into_raw(Box::new(builtins));
        ffi::duckdb_copy_function_set_extra_info(
            function.0,
            extra.cast(),
            Some(drop_box::<Arc<Builtins>>),
        );
        ffi::duckdb_copy_function_set_bind(function.0, Some(bind));
        ffi::duckdb_copy_function_set_sink(function.0, Some(sink));
    }
    // SAFETY: both handles are live; DuckDB copies the function into the
    // catalog, extra info included.
    let state = unsafe { ffi::duckdb_register_copy_function(connection, function.0) };
    if state == ffi::duckdb_state_DuckDBSuccess {
        Ok(())
    } else {
        Err(format!("the copy format {FORMAT} could not be registered"))
    }
}

/// A copy function handle, destroyed on drop.
struct CopyFunction(ffi::duckdb_copy_function);

impl CopyFunction {
    fn new() -> CopyFunction {
        // SAFETY: creating a copy function has no precondition.
        CopyFunction(unsafe { ffi::duckdb_create_copy_function() })
    }
}

impl Drop for CopyFunction {
    fn drop(&mut self) {
        // SAFETY: the handle was created by DuckDB and is destroyed once.
        unsafe { ffi::duckdb_destroy_copy_function(&mut self.0) };
    }
}

/// What the bind settled: the spec, the envelope and the input columns.
#[expect(dead_code, reason = "the rows are not written yet")]
struct Bound {
    builtins: Arc<Builtins>,
    version: &'static str,
    envelope: Envelope,
    columns: Vec<BoundColumn>,
}

/// Runs `body`, turning a panic into an error.
fn guarded<T>(body: impl FnOnce() -> Result<T, CopyError>) -> Result<T, CopyError> {
    catch_unwind(AssertUnwindSafe(body)).unwrap_or_else(|payload| {
        Err(CopyError::Internal {
            message: panic_message(payload.as_ref()),
        })
    })
}

/// The error's message as a C string; a NUL byte is written `\0`.
fn c_message(error: &CopyError) -> CString {
    let text = error.to_string().replace('\0', "\\0");
    CString::new(text).unwrap_or_default()
}

fn missing(what: &str) -> CopyError {
    CopyError::Internal {
        message: format!("DuckDB gave no {what}"),
    }
}

unsafe extern "C" fn bind(info: ffi::duckdb_copy_function_bind_info) {
    // SAFETY: DuckDB passes a live bind info to the bind callback.
    if let Err(error) = guarded(|| unsafe { bind_with(info) }) {
        let message = c_message(&error);
        // SAFETY: as above; DuckDB copies the message.
        unsafe { ffi::duckdb_copy_function_bind_set_error(info, message.as_ptr()) };
    }
}

/// # Safety
///
/// `info` must be the live bind info of the `edi835` copy format.
unsafe fn bind_with(info: ffi::duckdb_copy_function_bind_info) -> Result<(), CopyError> {
    // SAFETY: the extra info is the `Arc<Builtins>` set at registration,
    // alive as long as the function.
    let builtins = unsafe {
        ffi::duckdb_copy_function_bind_get_extra_info(info)
            .cast::<Arc<Builtins>>()
            .as_ref()
    }
    .ok_or_else(|| missing("extra info"))?;
    // SAFETY: `info` is the live bind info (caller contract).
    let given = unsafe { read::options(info) };
    let settings = Settings::parse(&given)?;
    let builtin =
        builtins
            .by_version(&settings.version)
            .ok_or_else(|| CopyError::UnknownVersion {
                version: settings.version.clone(),
                known: builtins.versions(),
            })?;
    // SAFETY: `info` is live (caller contract).
    let count = unsafe { ffi::duckdb_copy_function_bind_get_column_count(info) };
    let columns = (0..count)
        .map(|index| {
            // SAFETY: `index` is below the column count; the type returned is
            // owned here and destroyed after it is read.
            unsafe {
                let mut kind = ffi::duckdb_copy_function_bind_get_column_type(info, index);
                if kind.is_null() {
                    return Err(missing("column type"));
                }
                let column = input::column_of(kind);
                ffi::duckdb_destroy_logical_type(&mut kind);
                Ok(column)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let columns = input::bind(&builtin.tables, &columns)?;
    let bound = Bound {
        builtins: Arc::clone(builtins),
        version: builtin.version,
        envelope: settings.envelope,
        columns,
    };
    // SAFETY: `info` is live; DuckDB owns the box from here and frees it
    // with `drop_box::<Bound>`.
    unsafe {
        ffi::duckdb_copy_function_bind_set_bind_data(
            info,
            Box::into_raw(Box::new(bound)).cast(),
            Some(drop_box::<Bound>),
        );
    }
    Ok(())
}

unsafe extern "C" fn sink(info: ffi::duckdb_copy_function_sink_info, _: ffi::duckdb_data_chunk) {
    let error = CopyError::Internal {
        message: "the rows cannot be written yet".to_owned(),
    };
    let message = c_message(&error);
    // SAFETY: DuckDB passes a live info and copies the message.
    unsafe { ffi::duckdb_copy_function_sink_set_error(info, message.as_ptr()) };
}

#[cfg(test)]
mod tests;
