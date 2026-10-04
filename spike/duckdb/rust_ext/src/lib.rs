//! A DuckDB loadable extension over `edi835_core`.
//!
//! `read_835(path, table_name := 'claims')` parses one 835 file with the built-in
//! spec and returns one of its projected tables, column by column, with the
//! column types the core declares.
//!
//! `oxedi835_rows_of(table_name)` reads every row of another table from inside
//! the extension through a second connection to the same database; it is the
//! plumbing a writer that receives table names would need.
//!
//! `COPY (...) TO 'file' (FORMAT oxedi835_probe)` receives the rows of one
//! query and writes their count; it is the plumbing of a copy format.

use std::error::Error;
use std::ffi::{CStr, CString, c_void};
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use duckdb::core::{DataChunkHandle, FlatVector, Inserter, LogicalTypeHandle, LogicalTypeId};
use duckdb::ffi::{self, duckdb_hugeint, duckdb_vector_size};
use duckdb::vtab::{BindInfo, InitInfo, TableFunctionInfo, VTab};
use duckdb::Connection;
use edi835_core::{Column, ColumnData, Document, Processor, Spec, Tables};

/// A connection to the database that loaded the extension.
static CONNECTION: OnceLock<Mutex<Connection>> = OnceLock::new();

/// The DuckDB type of one core column.
fn logical_type(data: &ColumnData) -> LogicalTypeHandle {
    match data.column() {
        Column::Binary { .. } => LogicalTypeId::Blob.into(),
        Column::Int64 { .. } => LogicalTypeId::Bigint.into(),
        Column::Decimal128 {
            precision, scale, ..
        } => LogicalTypeHandle::decimal(*precision, *scale),
        Column::Date32(_) => LogicalTypeId::Date.into(),
        Column::Time32(_) => LogicalTypeId::Time.into(),
    }
}

struct ReadBind {
    tables: Tables,
    table: usize,
}

struct ReadInit {
    next_row: AtomicUsize,
}

struct Read835;

impl VTab for Read835 {
    type BindData = ReadBind;
    type InitData = ReadInit;

    fn bind(bind: &BindInfo) -> Result<ReadBind, Box<dyn Error>> {
        let path = bind.get_parameter(0).to_string();
        let wanted = bind
            .get_named_parameter("table_name")
            .map(|value| value.to_string())
            .unwrap_or_else(|| "claims".to_owned());
        let bytes = std::fs::read(&path).map_err(|err| format!("read_835: {path}: {err}"))?;
        let document = Document::parse(bytes).map_err(|err| format!("read_835: {path}: {err}"))?;
        let spec = Spec::builtin_835();
        let (tables, _diagnostics) = Processor::run(&spec, &document);
        let table = tables
            .iter()
            .position(|table| table.name() == wanted)
            .ok_or_else(|| {
                let names: Vec<&str> = tables.iter().map(|table| table.name()).collect();
                format!("read_835: no table {wanted:?}; the spec projects {names:?}")
            })?;
        let Some(found) = tables.iter().nth(table) else {
            return Err("read_835: table index out of range".into());
        };
        for (name, data) in found.columns() {
            bind.add_result_column(name, logical_type(data));
        }
        bind.set_cardinality(found.len() as u64, true);
        Ok(ReadBind { tables, table })
    }

    fn init(_: &InitInfo) -> Result<ReadInit, Box<dyn Error>> {
        Ok(ReadInit {
            next_row: AtomicUsize::new(0),
        })
    }

    fn func(
        func: &TableFunctionInfo<Self>,
        output: &mut DataChunkHandle,
    ) -> Result<(), Box<dyn Error>> {
        let bind = func.get_bind_data();
        let Some(table) = bind.tables.iter().nth(bind.table) else {
            return Err("read_835: table index out of range".into());
        };
        let capacity = unsafe { duckdb_vector_size() } as usize;
        let start = func
            .get_init_data()
            .next_row
            .fetch_add(capacity, Ordering::Relaxed);
        let end = table.len().min(start.saturating_add(capacity));
        if start >= end {
            output.set_len(0);
            return Ok(());
        }
        for (index, (_, data)) in table.columns().iter().enumerate() {
            let mut vector = output.flat_vector(index);
            fill(&mut vector, data, start, end);
        }
        output.set_len(end - start);
        Ok(())
    }

    fn parameters() -> Option<Vec<LogicalTypeHandle>> {
        Some(vec![LogicalTypeId::Varchar.into()])
    }

    fn named_parameters() -> Option<Vec<(String, LogicalTypeHandle)>> {
        Some(vec![("table_name".to_owned(), LogicalTypeId::Varchar.into())])
    }
}

/// Copies rows `start..end` of one column into a DuckDB vector.
fn fill(vector: &mut FlatVector<'_>, data: &ColumnData, start: usize, end: usize) {
    let rows = end - start;
    match data.column() {
        Column::Binary { offsets, data: bytes } => {
            for row in start..end {
                let (Some(&from), Some(&to)) = (offsets.get(row), offsets.get(row + 1)) else {
                    continue;
                };
                let value = bytes.get(from as usize..to as usize).unwrap_or_default();
                vector.insert(row - start, value);
            }
        }
        Column::Int64 { values, .. } => {
            let out = unsafe { vector.as_mut_slice_with_len::<i64>(rows) };
            out.copy_from_slice(&values[start..end]);
        }
        Column::Decimal128 {
            values, precision, ..
        } => match precision {
            0..=4 => {
                let out = unsafe { vector.as_mut_slice_with_len::<i16>(rows) };
                for (slot, value) in out.iter_mut().zip(&values[start..end]) {
                    *slot = *value as i16;
                }
            }
            5..=9 => {
                let out = unsafe { vector.as_mut_slice_with_len::<i32>(rows) };
                for (slot, value) in out.iter_mut().zip(&values[start..end]) {
                    *slot = *value as i32;
                }
            }
            10..=18 => {
                let out = unsafe { vector.as_mut_slice_with_len::<i64>(rows) };
                for (slot, value) in out.iter_mut().zip(&values[start..end]) {
                    *slot = *value as i64;
                }
            }
            _ => {
                let out = unsafe { vector.as_mut_slice_with_len::<duckdb_hugeint>(rows) };
                for (slot, value) in out.iter_mut().zip(&values[start..end]) {
                    *slot = duckdb_hugeint {
                        lower: *value as u64,
                        upper: (*value >> 64) as i64,
                    };
                }
            }
        },
        Column::Date32(values) => {
            let out = unsafe { vector.as_mut_slice_with_len::<i32>(rows) };
            out.copy_from_slice(&values[start..end]);
        }
        Column::Time32(values) => {
            let out = unsafe { vector.as_mut_slice_with_len::<i64>(rows) };
            for (slot, value) in out.iter_mut().zip(&values[start..end]) {
                *slot = i64::from(*value) * 1_000_000;
            }
        }
    }
    if data.null_count() > 0 {
        for row in start..end {
            if data.validity().get(row) == Some(false) {
                vector.set_null(row - start);
            }
        }
    }
}

struct RowsBind {
    name: String,
    rows: i64,
    checksum: String,
}

struct RowsInit {
    done: AtomicUsize,
}

/// Reads all rows of a table named by the caller, from inside the extension.
struct RowsOf;

impl VTab for RowsOf {
    type BindData = RowsBind;
    type InitData = RowsInit;

    fn bind(bind: &BindInfo) -> Result<RowsBind, Box<dyn Error>> {
        let name = bind.get_parameter(0).to_string();
        let connection = CONNECTION
            .get()
            .ok_or("oxedi835_rows_of: no connection")?
            .lock()
            .map_err(|_| "oxedi835_rows_of: connection lock poisoned")?;
        let quoted = format!("\"{}\"", name.replace('"', "\"\""));
        let mut statement = connection.prepare(&format!("SELECT * FROM {quoted}"))?;
        let mut rows = statement.query([])?;
        let mut count = 0i64;
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        while let Some(row) = rows.next()? {
            count += 1;
            let first: duckdb::types::Value = row.get(0)?;
            for byte in format!("{first:?}").bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0100_0000_01b3);
            }
        }
        bind.add_result_column("table_name", LogicalTypeId::Varchar.into());
        bind.add_result_column("rows", LogicalTypeId::Bigint.into());
        bind.add_result_column("first_column_fnv", LogicalTypeId::Varchar.into());
        Ok(RowsBind {
            name,
            rows: count,
            checksum: format!("{hash:016x}"),
        })
    }

    fn init(_: &InitInfo) -> Result<RowsInit, Box<dyn Error>> {
        Ok(RowsInit {
            done: AtomicUsize::new(0),
        })
    }

    fn func(
        func: &TableFunctionInfo<Self>,
        output: &mut DataChunkHandle,
    ) -> Result<(), Box<dyn Error>> {
        if func.get_init_data().done.fetch_add(1, Ordering::Relaxed) > 0 {
            output.set_len(0);
            return Ok(());
        }
        let bind = func.get_bind_data();
        output.flat_vector(0).insert(0, bind.name.as_str());
        let mut rows = output.flat_vector(1);
        let slot = unsafe { rows.as_mut_slice_with_len::<i64>(1) };
        slot[0] = bind.rows;
        output.flat_vector(2).insert(0, bind.checksum.as_str());
        output.set_len(1);
        Ok(())
    }

    fn parameters() -> Option<Vec<LogicalTypeHandle>> {
        Some(vec![LogicalTypeId::Varchar.into()])
    }
}

/// State of one `COPY ... TO ... (FORMAT oxedi835_probe)`.
struct CopyState {
    path: String,
    columns: u64,
    rows: AtomicUsize,
}

unsafe extern "C" fn copy_bind(info: ffi::duckdb_copy_function_bind_info) {
    let columns = Box::new(unsafe { ffi::duckdb_copy_function_bind_get_column_count(info) });
    unsafe {
        ffi::duckdb_copy_function_bind_set_bind_data(
            info,
            Box::into_raw(columns).cast(),
            Some(drop_box::<u64>),
        );
    }
}

unsafe extern "C" fn copy_init(info: ffi::duckdb_copy_function_global_init_info) {
    let path = unsafe { CStr::from_ptr(ffi::duckdb_copy_function_global_init_get_file_path(info)) };
    let columns = unsafe { *ffi::duckdb_copy_function_global_init_get_bind_data(info).cast::<u64>() };
    let state = Box::new(CopyState {
        path: path.to_string_lossy().into_owned(),
        columns,
        rows: AtomicUsize::new(0),
    });
    unsafe {
        ffi::duckdb_copy_function_global_init_set_global_state(
            info,
            Box::into_raw(state).cast(),
            Some(drop_box::<CopyState>),
        );
    }
}

unsafe extern "C" fn copy_sink(info: ffi::duckdb_copy_function_sink_info, chunk: ffi::duckdb_data_chunk) {
    let state = unsafe { &*ffi::duckdb_copy_function_sink_get_global_state(info).cast::<CopyState>() };
    let rows = unsafe { ffi::duckdb_data_chunk_get_size(chunk) } as usize;
    state.rows.fetch_add(rows, Ordering::Relaxed);
}

unsafe extern "C" fn copy_finalize(info: ffi::duckdb_copy_function_finalize_info) {
    let state = unsafe { &*ffi::duckdb_copy_function_finalize_get_global_state(info).cast::<CopyState>() };
    let text = format!(
        "rows={} columns={}\n",
        state.rows.load(Ordering::Relaxed),
        state.columns
    );
    if let Err(err) = std::fs::write(&state.path, text) {
        let message = CString::new(format!("oxedi835_probe: {}: {err}", state.path)).unwrap_or_default();
        unsafe { ffi::duckdb_copy_function_finalize_set_error(info, message.as_ptr()) };
    }
}

unsafe extern "C" fn drop_box<T>(pointer: *mut c_void) {
    if !pointer.is_null() {
        drop(unsafe { Box::from_raw(pointer.cast::<T>()) });
    }
}

/// Registers `COPY ... TO 'file' (FORMAT oxedi835_probe)` on a raw connection.
unsafe fn register_copy(database: ffi::duckdb_database) -> Result<(), Box<dyn Error>> {
    unsafe {
        let mut connection: ffi::duckdb_connection = ptr::null_mut();
        if ffi::duckdb_connect(database, &mut connection) != ffi::DuckDBSuccess {
            return Err("oxedi835: could not connect to register the copy function".into());
        }
        let mut function = ffi::duckdb_create_copy_function();
        let name = CString::new("oxedi835_probe")?;
        ffi::duckdb_copy_function_set_name(function, name.as_ptr());
        ffi::duckdb_copy_function_set_bind(function, Some(copy_bind));
        ffi::duckdb_copy_function_set_global_init(function, Some(copy_init));
        ffi::duckdb_copy_function_set_sink(function, Some(copy_sink));
        ffi::duckdb_copy_function_set_finalize(function, Some(copy_finalize));
        let state = ffi::duckdb_register_copy_function(connection, function);
        ffi::duckdb_destroy_copy_function(&mut function);
        ffi::duckdb_disconnect(&mut connection);
        if state != ffi::DuckDBSuccess {
            return Err("oxedi835: the copy function was not registered".into());
        }
    }
    Ok(())
}

unsafe fn init(
    info: ffi::duckdb_extension_info,
    access: *const ffi::duckdb_extension_access,
) -> Result<bool, Box<dyn Error>> {
    unsafe {
        if !ffi::duckdb_rs_extension_api_init(info, access, MIN_DUCKDB_VERSION)? {
            return Ok(false);
        }
        let get_database = (*access).get_database.ok_or("no get_database in the access struct")?;
        let database_pointer = get_database(info);
        if database_pointer.is_null() {
            return Ok(false);
        }
        let database: ffi::duckdb_database = *database_pointer;
        let con = Connection::open_from_raw(database.cast())?;
        con.register_table_function::<Read835>("read_835")?;
        con.register_table_function::<RowsOf>("oxedi835_rows_of")?;
        let _ = CONNECTION.set(Mutex::new(con.try_clone()?));
        register_copy(database)?;
        Ok(true)
    }
}

/// The minimum DuckDB C API version the extension asks for.
const MIN_DUCKDB_VERSION: &str = match option_env!("DUCKDB_EXTENSION_MIN_DUCKDB_VERSION") {
    Some(version) => version,
    None => "v1.5.6",
};

/// Entry point DuckDB calls on `LOAD`.
///
/// # Safety
///
/// Called by DuckDB with valid `info` and `access` pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn oxedi835_init_c_api(
    info: ffi::duckdb_extension_info,
    access: *const ffi::duckdb_extension_access,
) -> bool {
    match unsafe { init(info, access) } {
        Ok(loaded) => loaded,
        Err(err) => {
            if let Some(set_error) = unsafe { (*access).set_error } {
                let message = CString::new(err.to_string()).unwrap_or_default();
                unsafe { set_error(info, message.as_ptr()) };
            }
            false
        }
    }
}
