//! Expanding glob patterns with DuckDB's `glob` table function.
//!
//! The stable C API offers no glob and no way to run SQL on the caller's
//! connection, and a connection to the caller's database kept past the
//! extension load would keep that database alive for the life of the
//! process. A pattern is therefore expanded by a private in-memory DuckDB
//! opened for the bind and closed after it: it lists files through DuckDB's
//! file systems (local, and remote ones its autoloaded extensions provide),
//! while the files themselves are read through the caller's file system.

use duckdb::{Config, Connection};

use crate::error::ReadError;

/// Whether `path` is a pattern DuckDB would expand: it holds `*`, `?` or `[`.
pub fn is_pattern(path: &str) -> bool {
    path.contains(['*', '?', '['])
}

/// Every path of `paths`, with each pattern replaced by the files it
/// matches, sorted. A pattern that matches nothing is an error.
pub fn resolve(paths: &[String]) -> Result<Vec<String>, ReadError> {
    let mut files = Vec::with_capacity(paths.len());
    let mut globber: Option<Connection> = None;
    for path in paths {
        if !is_pattern(path) {
            files.push(path.clone());
            continue;
        }
        let connection = match globber.as_ref() {
            Some(connection) => connection,
            None => globber.insert(open(path)?),
        };
        let matched = expand(connection, path)?;
        if matched.is_empty() {
            return Err(ReadError::NoFiles {
                pattern: path.clone(),
            });
        }
        files.extend(matched);
    }
    Ok(files)
}

/// A private single-threaded in-memory database.
fn open(pattern: &str) -> Result<Connection, ReadError> {
    Config::default()
        .threads(1)
        .and_then(Connection::open_in_memory_with_flags)
        .map_err(|err| ReadError::Glob {
            pattern: pattern.to_owned(),
            message: err.to_string(),
        })
}

/// The files `pattern` matches, sorted.
fn expand(connection: &Connection, pattern: &str) -> Result<Vec<String>, ReadError> {
    let failed = |err: duckdb::Error| ReadError::Glob {
        pattern: pattern.to_owned(),
        message: err.to_string(),
    };
    let literal = pattern.replace('\'', "''");
    let mut statement = connection
        .prepare(&format!("SELECT file FROM glob('{literal}') ORDER BY file"))
        .map_err(failed)?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(failed)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(failed)
}
