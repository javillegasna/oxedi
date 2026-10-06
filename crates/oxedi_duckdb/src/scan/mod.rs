//! The scan of `read_835`: one file at a time is read, parsed and kept
//! until all its rows of the bound table have been emitted, in chunks of at
//! most DuckDB's vector size.
//!
//! - `mod.rs`: what the bind produced ([`Bound`]) and the scan state
//!   ([`Scan`]).
//! - `write.rs`: copying a core column into a DuckDB vector.

mod write;

use std::sync::Arc;

use libduckdb_sys as ffi;
use oxedi_core::{ColumnType, Document, Processor, Table};

use crate::builtins::Builtins;
use crate::diagnostics;
use crate::error::ReadError;
use crate::files::FileSystem;
use crate::schema::SqlType;
use write::{Place, Vector};

/// The column every table carries with each row's ordinal.
const ROW_COLUMN: &str = "row";

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
    /// Whether the table is `diagnostics`, which every spec shares.
    pub diagnostics: bool,
    /// Whether a file that cannot be parsed is reported instead of failing:
    /// as one row of `diagnostics`, and as no rows of any other table.
    pub ignore_errors: bool,
    /// Whether text columns are BLOB instead of VARCHAR.
    pub binary: bool,
}

/// The bound table of the file being emitted.
#[derive(Debug)]
struct Loaded {
    /// Index of the file in [`Bound::files`].
    file: usize,
    table: Table,
    /// Rows already emitted.
    emitted: usize,
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
                Some(loaded) => loaded.table.len().saturating_sub(loaded.emitted),
                None => 0,
            };
            if pending > 0 {
                break;
            }
            let Some(file) = bound.files.get(self.next_file) else {
                self.current = None;
                return Ok(0);
            };
            let table = load(bound, file)?;
            self.current = Some(Loaded {
                file: self.next_file,
                table,
                emitted: 0,
            });
            self.next_file += 1;
        }
        let Some(loaded) = self.current.as_mut() else {
            return Ok(0);
        };
        let table = &loaded.table;
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
                rows: table.column(ROW_COLUMN),
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

/// Reads and parses one file, checks that its spec projects the bound table
/// with the bound columns, and keeps that table only (or its findings, for
/// `diagnostics`).
fn load(bound: &Bound, file: &str) -> Result<Table, ReadError> {
    let bytes = bound.file_system.read_all(file)?;
    let document = match Document::parse(bytes.as_slice()) {
        Ok(document) => document,
        Err(source) if bound.ignore_errors => {
            if !bound.diagnostics {
                return Ok(Table::new(bound.table.clone(), bound.columns.clone()));
            }
            let datum = diagnostics::datum(&source, &bytes).to_vec();
            let error = ReadError::Parse {
                file: file.to_owned(),
                source,
            };
            return diagnostics::of_unparsable(&error, &datum, bound.binary);
        }
        Err(source) => {
            return Err(ReadError::Parse {
                file: file.to_owned(),
                source,
            });
        }
    };
    let builtin = if bound.forced {
        bound.builtins.by_version(bound.bound_version)
    } else {
        Some(bound.builtins.select(document.segments()))
    }
    .ok_or_else(|| ReadError::Internal {
        message: format!(
            "{file:?}: no built-in spec for version {}",
            bound.bound_version
        ),
    })?;
    if bound.diagnostics {
        let (_tables, findings) = Processor::run(&builtin.spec, &document);
        return diagnostics::of_findings(&findings);
    }
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
    // The other tables and the document are dropped on return.
    tables
        .get(&bound.table)
        .cloned()
        .ok_or_else(|| ReadError::Internal {
            message: format!("{file:?}: the parsed file has no table {:?}", bound.table),
        })
}

#[cfg(test)]
mod tests;
