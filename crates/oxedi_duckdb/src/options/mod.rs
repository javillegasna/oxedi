//! The arguments of `read_835`, read from the bind info.

use std::ffi::CString;

use libduckdb_sys as ffi;

use crate::error::ReadError;
use crate::value::Value;

/// The table read when `table_name` is not given.
pub const DEFAULT_TABLE: &str = "claims";

/// The named parameters, in registration order, with whether each is a
/// boolean (else a VARCHAR).
pub const NAMED: [(&str, bool); 5] = [
    ("table_name", false),
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
    pub table_name: String,
    /// Whether to add a `filename` column.
    pub filename: bool,
    /// The forced spec version, if any.
    pub version: Option<String>,
    /// Whether text columns are `BLOB` instead of `VARCHAR`.
    pub binary: bool,
    /// Whether a file that cannot be parsed is reported instead of failing.
    pub ignore_errors: bool,
}

/// A named parameter as the call gave it.
enum Named {
    /// Not given.
    Absent,
    /// Given as SQL NULL.
    Null,
    /// Given with a value.
    Given(Value),
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
        // contract); the value is owned by the caller and taken by `owned`.
        let path = unsafe { Value::owned(ffi::duckdb_bind_get_parameter(info, 0)) };
        let paths = match path {
            Some(path) => paths_of(&path)?,
            None => return Err(ReadError::NullPath { position: None }),
        };
        // SAFETY: each name below is one of NAMED (caller contract).
        let named = |name: &str| unsafe { named(info, name) };
        let flag = |name: &'static str| match named(name) {
            Named::Absent => Ok(false),
            Named::Null => Err(ReadError::NullOption { name }),
            Named::Given(value) => Ok(value.boolean()),
        };
        Ok(Options {
            paths,
            table_name: match named("table_name") {
                Named::Absent => DEFAULT_TABLE.to_owned(),
                Named::Null => return Err(ReadError::NullOption { name: "table_name" }),
                Named::Given(value) => value.text(),
            },
            filename: flag("filename")?,
            version: match named("version") {
                Named::Absent | Named::Null => None,
                Named::Given(value) => Some(value.text()),
            },
            binary: flag("binary")?,
            ignore_errors: flag("ignore_errors")?,
        })
    }
}

/// The named parameter `name` of the bind.
///
/// # Safety
///
/// `info` must be a live bind info.
unsafe fn named(info: ffi::duckdb_bind_info, name: &str) -> Named {
    let Ok(name) = CString::new(name) else {
        return Named::Absent;
    };
    // SAFETY: `info` is live (caller contract) and `name` is a C string; the
    // value returned (null when the parameter was not given) is owned here.
    let raw = unsafe { ffi::duckdb_bind_get_named_parameter(info, name.as_ptr()) };
    if raw.is_null() {
        return Named::Absent;
    }
    // SAFETY: `raw` is a live value handle owned here.
    match unsafe { Value::owned(raw) } {
        Some(value) => Named::Given(value),
        None => Named::Null,
    }
}

/// The paths of a VARCHAR or a list of VARCHAR.
fn paths_of(value: &Value) -> Result<Vec<String>, ReadError> {
    match value.type_id() {
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_VARCHAR => Ok(vec![value.text()]),
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_LIST => {
            let items = value.list();
            if items.is_empty() {
                return Err(ReadError::EmptyPathList);
            }
            items
                .iter()
                .enumerate()
                .map(|(index, item)| match item {
                    None => Err(ReadError::NullPath {
                        position: Some(index + 1),
                    }),
                    Some(item) if item.type_id() == ffi::DUCKDB_TYPE_DUCKDB_TYPE_VARCHAR => {
                        Ok(item.text())
                    }
                    Some(_) => Err(ReadError::PathType {
                        found: value.type_name(),
                    }),
                })
                .collect()
        }
        _ => Err(ReadError::PathType {
            found: value.type_name(),
        }),
    }
}
