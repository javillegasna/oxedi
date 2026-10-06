//! The scan of `read_835`: one file at a time is read, parsed and kept
//! until all its rows of the bound table have been emitted, in chunks of at
//! most DuckDB's vector size.
//!
//! - `mod.rs`: what the bind produced ([`Bound`]) and the scan state
//!   ([`Scan`]).
//! - `write.rs`: copying a core column into a DuckDB vector.

mod write;

use std::sync::Arc;

use duckdb::ffi;
use edi835_core::{ColumnType, Document, Processor, Table, Tables};

use crate::builtins::Builtins;
use crate::error::ReadError;
use crate::files::FileSystem;
use crate::schema::SqlType;
use write::{Place, Vector};

/// Everything the bind settled, read by the scan.
#[derive(Debug)]
pub struct Bound {
    /// The files to read, in order.
    pub files: Vec<String>,
    /// The client context's file system.
    pub file_system: FileSystem,
    /// The built-in specs.
    pub builtins: Arc<Builtins>,
    /// The table to emit.
    pub table: String,
    /// The table's columns as bound: name and core type.
    pub columns: Vec<(String, ColumnType)>,
    /// The DuckDB type of each column.
    pub types: Vec<SqlType>,
    /// The version whose spec the columns were bound with.
    pub bound_version: &'static str,
    /// Whether every file uses the bound version instead of the one it
    /// declares.
    pub forced: bool,
    /// Whether a `filename` column follows the table's columns.
    pub filename: bool,
}

/// The tables of the file being emitted.
#[derive(Debug)]
struct Loaded {
    /// Index of the file in [`Bound::files`].
    file: usize,
    tables: Tables,
    /// Rows already emitted.
    emitted: usize,
}

impl Loaded {
    fn table<'a>(&'a self, bound: &Bound) -> Result<&'a Table, ReadError> {
        self.tables
            .get(&bound.table)
            .ok_or_else(|| ReadError::Internal {
                message: format!("the parsed file has no table {:?}", bound.table),
            })
    }
}

/// Where the scan is.
#[derive(Debug, Default)]
pub struct Scan {
    /// Index of the next file to load.
    next_file: usize,
    current: Option<Loaded>,
}

impl Scan {
    /// Fills `output` with the next rows, or leaves it empty when every file
    /// has been emitted.
    ///
    /// # Safety
    ///
    /// `output` must be the chunk DuckDB passed to the running scan callback
    /// of the bind that produced `bound`.
    pub unsafe fn next_chunk(
        &mut self,
        bound: &Bound,
        output: ffi::duckdb_data_chunk,
    ) -> Result<usize, ReadError> {
        // SAFETY: reading the vector size has no precondition.
        let capacity = usize::try_from(unsafe { ffi::duckdb_vector_size() }).unwrap_or(0);
        loop {
            let pending = match &self.current {
                Some(loaded) => loaded.table(bound)?.len().saturating_sub(loaded.emitted),
                None => 0,
            };
            if pending > 0 {
                break;
            }
            let Some(file) = bound.files.get(self.next_file) else {
                self.current = None;
                return Ok(0);
            };
            let tables = load(bound, file)?;
            self.current = Some(Loaded {
                file: self.next_file,
                tables,
                emitted: 0,
            });
            self.next_file += 1;
        }
        let Some(loaded) = self.current.as_mut() else {
            return Ok(0);
        };
        let table = loaded.table(bound)?;
        let start = loaded.emitted;
        let len = table.len().saturating_sub(start).min(capacity);
        let file = bound.files.get(loaded.file).map_or("", String::as_str);
        for (index, ((name, data), sql)) in table.columns().iter().zip(&bound.types).enumerate() {
            // SAFETY: `output` is the scan's chunk (caller contract), whose
            // columns are the bound ones: table columns first.
            let mut vector = unsafe { Vector::of(output, index, capacity) };
            let place = Place {
                file,
                table: &bound.table,
                column: name,
            };
            vector.fill(data, *sql, start, len, &place)?;
        }
        if bound.filename {
            // SAFETY: as above; the `filename` column follows the table's.
            let mut vector = unsafe { Vector::of(output, bound.columns.len(), capacity) };
            vector.fill_text(len, file);
        }
        loaded.emitted = start + len;
        Ok(len)
    }
}

/// Reads and parses one file, and checks that its spec projects the bound
/// table with the bound columns.
fn load(bound: &Bound, file: &str) -> Result<Tables, ReadError> {
    let bytes = bound.file_system.read_all(file)?;
    let document = Document::parse(bytes).map_err(|source| ReadError::Parse {
        file: file.to_owned(),
        source,
    })?;
    let builtin = if bound.forced {
        bound.builtins.by_version(bound.bound_version)
    } else {
        Some(bound.builtins.select(document.segments()))
    }
    .ok_or_else(|| ReadError::Internal {
        message: format!("no built-in spec for version {}", bound.bound_version),
    })?;
    let same = builtin
        .table(&bound.table)
        .is_some_and(|table| table.columns == bound.columns);
    if !same {
        return Err(ReadError::SchemaMismatch {
            file: file.to_owned(),
            table: bound.table.clone(),
            version: builtin.version,
            bound: bound.bound_version,
        });
    }
    let (tables, _diagnostics) = Processor::run(&builtin.spec, &document);
    Ok(tables)
}

#[cfg(test)]
mod tests;
