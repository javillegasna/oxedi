//! The arguments of `read_835`, read from the bind info.

use std::ffi::CString;

use duckdb::core::LogicalTypeId;
use duckdb::ffi;
use duckdb::vtab::Value;

use crate::error::ReadError;

/// The table read when `table` is not given.
pub const DEFAULT_TABLE: &str = "claims";

/// The named parameters, in registration order, with whether each is a
/// boolean (else a VARCHAR).
pub const NAMED: [(&str, bool); 5] = [
    ("table", false),
    ("filename", true),
    ("version", false),
    ("binary", true),
    ("ignore_errors", true),
];

/// What one call of `read_835` asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// The paths as given: plain paths or glob patterns.
    pub paths: Vec<String>,
    /// The table to return.
    pub table: String,
    /// Whether to add a `filename` column.
    pub filename: bool,
    /// The forced spec version, if any.
    pub version: Option<String>,
    /// Whether text columns are `BLOB` instead of `VARCHAR`.
    pub binary: bool,
    /// Whether a file that cannot be parsed is reported instead of failing.
    pub ignore_errors: bool,
}

impl Options {
    /// Reads the arguments of the bind in progress.
    ///
    /// # Safety
    ///
    /// `info` must be the bind info DuckDB passed to the running bind
    /// callback of a function registered with one positional parameter and
    /// the named parameters of [`NAMED`].
    pub unsafe fn of_bind(info: ffi::duckdb_bind_info) -> Result<Options, ReadError> {
        // SAFETY: index 0 is the one positional parameter (caller
        // contract); the value is owned and destroyed by `Value`.
        let path = unsafe { owned(ffi::duckdb_bind_get_parameter(info, 0)) };
        let paths = match path {
            Some(path) => paths_of(&path)?,
            None => return Err(ReadError::NullPath { position: None }),
        };
        // SAFETY: each name below is one of NAMED (caller contract).
        let named = |name: &str| unsafe { named(info, name) };
        Ok(Options {
            paths,
            table: named("table").map_or_else(|| DEFAULT_TABLE.to_owned(), |v| v.to_string()),
            filename: named("filename").is_some_and(|v| v.to_bool()),
            version: named("version").map(|v| v.to_string()),
            binary: named("binary").is_some_and(|v| v.to_bool()),
            ignore_errors: named("ignore_errors").is_some_and(|v| v.to_bool()),
        })
    }
}

/// The value DuckDB returned, or `None` for a missing or NULL value.
///
/// # Safety
///
/// `raw` must be null or a value handle the caller owns.
unsafe fn owned(raw: ffi::duckdb_value) -> Option<Value> {
    if raw.is_null() {
        return None;
    }
    let value = Value::from(raw);
    (!value.is_null()).then_some(value)
}

/// The named parameter `name`, or `None` when it was not given or is NULL.
///
/// # Safety
///
/// `info` must be a live bind info.
unsafe fn named(info: ffi::duckdb_bind_info, name: &str) -> Option<Value> {
    let name = CString::new(name).ok()?;
    // SAFETY: `info` is live (caller contract) and `name` is a C string; the
    // value returned is owned by the caller.
    unsafe { owned(ffi::duckdb_bind_get_named_parameter(info, name.as_ptr())) }
}

/// The paths of a VARCHAR or a list of VARCHAR.
fn paths_of(value: &Value) -> Result<Vec<String>, ReadError> {
    match value.logical_type_id() {
        LogicalTypeId::Varchar => Ok(vec![value.to_string()]),
        LogicalTypeId::List => {
            let items = value.to_list().unwrap_or_default();
            if items.is_empty() {
                return Err(ReadError::EmptyPathList);
            }
            items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    if item.is_null() {
                        return Err(ReadError::NullPath {
                            position: Some(index + 1),
                        });
                    }
                    match item.logical_type_id() {
                        LogicalTypeId::Varchar => Ok(item.to_string()),
                        other => Err(ReadError::PathType {
                            found: format!("a list of {other:?}"),
                        }),
                    }
                })
                .collect()
        }
        other => Err(ReadError::PathType {
            found: format!("{other:?}"),
        }),
    }
}
