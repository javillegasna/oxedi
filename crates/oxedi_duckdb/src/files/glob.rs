//! Expanding glob patterns with DuckDB's `glob` table function.
//!
//! The stable C API offers no glob and no way to run SQL on the caller's
//! connection, and a connection to the caller's database kept past the
//! extension load would keep that database alive for the life of the
//! process. A pattern is therefore expanded by a private in-memory DuckDB
//! opened for the bind and closed after it. That database follows the
//! caller's settings: with `enable_external_access` off no pattern is
//! expanded at all; otherwise it copies the caller's allowed directories and
//! paths, extension autoinstall and autoload, and its persistent-secret
//! settings (`allow_persistent_secrets`, `secret_directory`), so a remote
//! pattern lists files with the caller's persistent secrets and no other
//! ones. Temporary secrets (`CREATE SECRET` without `PERSISTENT`) live in the
//! caller's database only and do not reach the private one. The files
//! themselves are read through the caller's file system.

use std::ffi::{CStr, CString, c_char};

use libduckdb_sys as ffi;

use crate::error::ReadError;
use crate::files::ClientContext;
use crate::value::Value;

/// The caller's settings the private database is opened with.
const CONFIGURED: [&str; 4] = [
    "autoinstall_known_extensions",
    "autoload_known_extensions",
    "allow_persistent_secrets",
    "secret_directory",
];

/// The caller's list settings the private database takes once started:
/// DuckDB refuses them in the configuration it opens with.
const SET_AFTER_START: [&str; 2] = ["allowed_directories", "allowed_paths"];

/// The query that lists the files a pattern matches.
const GLOB_QUERY: &CStr = c"SELECT file FROM glob($1) ORDER BY file";

/// Whether `path` is a pattern DuckDB would expand: it holds `*`, `?` or
/// `[`, and is not an `http://` or `https://` URL (whose `?` starts a query).
pub fn is_pattern(path: &str) -> bool {
    let url = ["http://", "https://"].iter().any(|scheme| {
        path.get(..scheme.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(scheme))
    });
    !url && path.contains(['*', '?', '['])
}

/// The caller's settings that decide how patterns are expanded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallerSettings {
    /// The caller's `enable_external_access`.
    pub external_access: bool,
    /// Each configured setting the caller has, as configuration text.
    pub configured: Vec<(&'static str, String)>,
    /// Each non-empty list setting the caller has, as an SQL list literal.
    pub lists: Vec<(&'static str, String)>,
}

impl CallerSettings {
    /// Reads the settings of the client context.
    pub fn of(context: &ClientContext) -> CallerSettings {
        let setting = |name: &str| -> Option<Value> {
            let name = CString::new(name).ok()?;
            // SAFETY: the context is live; the value returned (null when the
            // setting does not exist) is owned here.
            unsafe {
                Value::owned(ffi::duckdb_client_context_get_config_option(
                    context.raw(),
                    name.as_ptr(),
                    std::ptr::null_mut(),
                ))
            }
        };
        CallerSettings {
            external_access: setting("enable_external_access").is_none_or(|v| v.boolean()),
            configured: CONFIGURED
                .iter()
                .filter_map(|name| setting(name).map(|value| (*name, value.text())))
                .collect(),
            lists: SET_AFTER_START
                .iter()
                .filter_map(|name| {
                    let value = setting(name)?;
                    let items = value.list();
                    (!items.is_empty()).then(|| (*name, list_literal(&items)))
                })
                .collect(),
        }
    }
}

/// List items as an SQL list literal of strings: `['a', 'b''c']`.
fn list_literal(items: &[Option<Value>]) -> String {
    let items: Vec<String> = items
        .iter()
        .flatten()
        .map(|item| format!("'{}'", item.text().replace('\'', "''")))
        .collect();
    format!("[{}]", items.join(", "))
}

/// Every path of `paths`, with each pattern replaced by the files it
/// matches, sorted. A pattern that matches nothing is an error.
pub fn resolve(paths: &[String], settings: &CallerSettings) -> Result<Vec<String>, ReadError> {
    let mut files = Vec::with_capacity(paths.len());
    let mut globber: Option<Private> = None;
    for path in paths {
        if !is_pattern(path) {
            files.push(path.clone());
            continue;
        }
        if !settings.external_access {
            return Err(ReadError::PatternWithoutExternalAccess {
                pattern: path.clone(),
            });
        }
        let private = match globber.as_ref() {
            Some(private) => private,
            None => globber.insert(Private::open(settings, path)?),
        };
        let matched = private.glob(path)?;
        if matched.is_empty() {
            return Err(ReadError::NoFiles {
                pattern: path.clone(),
            });
        }
        files.extend(matched);
    }
    Ok(files)
}

/// A private in-memory database and one connection to it, closed on drop.
struct Private {
    database: ffi::duckdb_database,
    connection: ffi::duckdb_connection,
}

impl Private {
    fn open(settings: &CallerSettings, pattern: &str) -> Result<Private, ReadError> {
        let failed = |message: String| ReadError::Glob {
            pattern: pattern.to_owned(),
            message,
        };
        let config = Config::new().ok_or_else(|| failed("no configuration".to_owned()))?;
        config.set("threads", "1").map_err(failed)?;
        for (name, value) in &settings.configured {
            config.set(name, value).map_err(failed)?;
        }
        let mut private = Private {
            database: std::ptr::null_mut(),
            connection: std::ptr::null_mut(),
        };
        let mut error: *mut c_char = std::ptr::null_mut();
        // SAFETY: a null path opens an in-memory database; DuckDB writes the
        // database (or an error string the caller frees) into the out
        // pointers.
        let state = unsafe {
            ffi::duckdb_open_ext(
                std::ptr::null(),
                &mut private.database,
                config.0,
                &mut error,
            )
        };
        if state != ffi::duckdb_state_DuckDBSuccess {
            // SAFETY: `error` is null or a string DuckDB allocated for us.
            return Err(failed(unsafe { take_string(error) }));
        }
        // SAFETY: the database was opened above; DuckDB writes a new
        // connection into `private.connection`.
        if unsafe { ffi::duckdb_connect(private.database, &mut private.connection) }
            != ffi::duckdb_state_DuckDBSuccess
        {
            return Err(failed("no connection to the private database".to_owned()));
        }
        for (name, list) in &settings.lists {
            private
                .execute(&format!("SET {name} = {list}"))
                .map_err(failed)?;
        }
        Ok(private)
    }

    /// Runs one statement and discards its result.
    fn execute(&self, sql: &str) -> Result<(), String> {
        let c_sql = CString::new(sql).map_err(|_| format!("{sql:?} holds a NUL byte"))?;
        let mut result = QueryResult::new();
        // SAFETY: the connection is live and `c_sql` a C string; DuckDB fills
        // `result`, destroyed by `QueryResult` whatever the outcome.
        let state = unsafe { ffi::duckdb_query(self.connection, c_sql.as_ptr(), &mut result.0) };
        if state == ffi::duckdb_state_DuckDBSuccess {
            Ok(())
        } else {
            // SAFETY: `result` holds the failed query; its error is owned by it.
            Err(unsafe { borrowed_string(ffi::duckdb_result_error(&mut result.0)) })
        }
    }

    /// The files `pattern` matches, sorted.
    fn glob(&self, pattern: &str) -> Result<Vec<String>, ReadError> {
        let failed = |message: String| ReadError::Glob {
            pattern: pattern.to_owned(),
            message,
        };
        let statement = Prepared::new(self.connection).map_err(failed)?;
        statement.run(pattern).map_err(failed)
    }
}

impl Drop for Private {
    fn drop(&mut self) {
        // SAFETY: both handles are null or were created by DuckDB, and each
        // is closed once, the connection first.
        unsafe {
            ffi::duckdb_disconnect(&mut self.connection);
            ffi::duckdb_close(&mut self.database);
        }
    }
}

/// A database configuration, destroyed on drop.
struct Config(ffi::duckdb_config);

impl Config {
    fn new() -> Option<Config> {
        let mut config: ffi::duckdb_config = std::ptr::null_mut();
        // SAFETY: DuckDB writes a new configuration into `config`.
        let state = unsafe { ffi::duckdb_create_config(&mut config) };
        (state == ffi::duckdb_state_DuckDBSuccess && !config.is_null()).then_some(Config(config))
    }

    fn set(&self, name: &str, value: &str) -> Result<(), String> {
        let refused = || format!("the setting {name} = {value:?} was refused");
        let c_name = CString::new(name).map_err(|_| refused())?;
        let c_value = CString::new(value).map_err(|_| refused())?;
        // SAFETY: the configuration is live; DuckDB copies both strings.
        let state = unsafe { ffi::duckdb_set_config(self.0, c_name.as_ptr(), c_value.as_ptr()) };
        if state == ffi::duckdb_state_DuckDBSuccess {
            Ok(())
        } else {
            Err(refused())
        }
    }
}

impl Drop for Config {
    fn drop(&mut self) {
        // SAFETY: the configuration was created by DuckDB and is destroyed once.
        unsafe { ffi::duckdb_destroy_config(&mut self.0) };
    }
}

/// The prepared glob query, destroyed on drop.
struct Prepared(ffi::duckdb_prepared_statement);

impl Prepared {
    fn new(connection: ffi::duckdb_connection) -> Result<Prepared, String> {
        let mut statement = Prepared(std::ptr::null_mut());
        // SAFETY: the connection is live and the query a C string; DuckDB
        // writes a statement (also on failure, for its error) into the out
        // pointer.
        let state =
            unsafe { ffi::duckdb_prepare(connection, GLOB_QUERY.as_ptr(), &mut statement.0) };
        if state != ffi::duckdb_state_DuckDBSuccess {
            // SAFETY: the statement is live; its error string is owned by it.
            return Err(unsafe { borrowed_string(ffi::duckdb_prepare_error(statement.0)) });
        }
        Ok(statement)
    }

    /// Runs the query for `pattern` and collects the file column.
    fn run(&self, pattern: &str) -> Result<Vec<String>, String> {
        let bytes = pattern.as_bytes();
        // SAFETY: the statement is live; DuckDB copies the `bytes.len()`
        // bytes of the pattern.
        let state = unsafe {
            ffi::duckdb_bind_varchar_length(self.0, 1, bytes.as_ptr().cast(), bytes.len() as u64)
        };
        if state != ffi::duckdb_state_DuckDBSuccess {
            return Err("the pattern could not be bound".to_owned());
        }
        let mut result = QueryResult::new();
        // SAFETY: the statement is live; DuckDB fills `result`, which is
        // destroyed by `QueryResult` whether or not the query succeeded.
        let state = unsafe { ffi::duckdb_execute_prepared(self.0, &mut result.0) };
        if state != ffi::duckdb_state_DuckDBSuccess {
            // SAFETY: `result` holds the failed query; its error is owned by it.
            return Err(unsafe { borrowed_string(ffi::duckdb_result_error(&mut result.0)) });
        }
        result.first_column()
    }
}

impl Drop for Prepared {
    fn drop(&mut self) {
        // SAFETY: the statement is null or was created by DuckDB, and is
        // destroyed once.
        unsafe { ffi::duckdb_destroy_prepare(&mut self.0) };
    }
}

/// A materialized query result, destroyed on drop.
struct QueryResult(ffi::duckdb_result);

impl QueryResult {
    fn new() -> QueryResult {
        // SAFETY: `duckdb_result` is a plain C struct for which all-zero is
        // the empty state DuckDB expects before filling it.
        QueryResult(unsafe { std::mem::zeroed() })
    }

    /// Every non-NULL value of the first (VARCHAR) column, chunk by chunk.
    fn first_column(&mut self) -> Result<Vec<String>, String> {
        let mut values = Vec::new();
        loop {
            // SAFETY: the result is live; the chunk returned (null at the
            // end) is owned here and destroyed by `Chunk`.
            let chunk = Chunk(unsafe { ffi::duckdb_fetch_chunk(self.0) });
            if chunk.0.is_null() {
                return Ok(values);
            }
            chunk.read_varchar(&mut values)?;
        }
    }
}

impl Drop for QueryResult {
    fn drop(&mut self) {
        // SAFETY: the result is empty or was filled by DuckDB, and is
        // destroyed once.
        unsafe { ffi::duckdb_destroy_result(&mut self.0) };
    }
}

/// One fetched data chunk, destroyed on drop.
struct Chunk(ffi::duckdb_data_chunk);

impl Chunk {
    fn read_varchar(&self, values: &mut Vec<String>) -> Result<(), String> {
        // SAFETY: the chunk is live and has the query's single VARCHAR
        // column; its data holds `size` `duckdb_string_t`, and the validity
        // mask (null when every row is valid) covers them.
        unsafe {
            let size = ffi::duckdb_data_chunk_get_size(self.0);
            let vector = ffi::duckdb_data_chunk_get_vector(self.0, 0);
            if vector.is_null() {
                return Err("the glob result has no column".to_owned());
            }
            let data = ffi::duckdb_vector_get_data(vector).cast::<ffi::duckdb_string_t>();
            let validity = ffi::duckdb_vector_get_validity(vector);
            for row in 0..size {
                if !validity.is_null() && !ffi::duckdb_validity_row_is_valid(validity, row) {
                    continue;
                }
                let string = data.add(usize::try_from(row).map_err(|err| err.to_string())?);
                let length = ffi::duckdb_string_t_length(*string) as usize;
                let start = ffi::duckdb_string_t_data(string).cast::<u8>();
                let bytes = std::slice::from_raw_parts(start, length);
                values.push(String::from_utf8_lossy(bytes).into_owned());
            }
        }
        Ok(())
    }
}

impl Drop for Chunk {
    fn drop(&mut self) {
        // SAFETY: the chunk is null or was returned by DuckDB, and is
        // destroyed once.
        unsafe { ffi::duckdb_destroy_data_chunk(&mut self.0) };
    }
}

/// Copies and frees a string DuckDB allocated.
///
/// # Safety
///
/// `raw` must be null or a NUL-terminated string allocated by DuckDB.
unsafe fn take_string(raw: *mut c_char) -> String {
    if raw.is_null() {
        return "DuckDB gave no error message".to_owned();
    }
    // SAFETY: `raw` is a NUL-terminated DuckDB string (caller contract),
    // freed once after the copy.
    unsafe {
        let text = CStr::from_ptr(raw).to_string_lossy().into_owned();
        ffi::duckdb_free(raw.cast());
        text
    }
}

/// Copies a string DuckDB keeps ownership of.
///
/// # Safety
///
/// `raw` must be null or a NUL-terminated string valid during the call.
unsafe fn borrowed_string(raw: *const c_char) -> String {
    if raw.is_null() {
        return "DuckDB gave no error message".to_owned();
    }
    // SAFETY: `raw` is a live NUL-terminated string (caller contract).
    unsafe { CStr::from_ptr(raw) }
        .to_string_lossy()
        .into_owned()
}
