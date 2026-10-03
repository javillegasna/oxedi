//! `Tables` and `Table`: the projected tables, readable as text or as Arrow.

use std::fmt::Write as _;
use std::sync::Arc;

use edi835_core::{Table, Tables};
use pyo3::exceptions::{PyKeyError, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::types::{PyIterator, PyTuple};

/// Writes one table as text: a title with the row count, the header, one
/// line per row and an empty line.
fn render_table(table: &Table, out: &mut String) {
    let _ = writeln!(out, "## {} (rows: {})", table.name(), table.len());
    let header = table
        .columns()
        .iter()
        .map(|(name, column)| format!("{name}: {}", column.kind()))
        .collect::<Vec<_>>()
        .join(" | ");
    let _ = writeln!(out, "{header}");
    for row in 0..table.len() {
        let line = table
            .columns()
            .iter()
            .map(|(_, column)| column.render(row).unwrap_or_default())
            .collect::<Vec<_>>()
            .join(" | ");
        let _ = writeln!(out, "{line}");
    }
    out.push('\n');
}

/// The tables of one parse or one batch, by name. Shared, never copied.
#[pyclass(name = "Tables", module = "oxedi835", frozen, mapping)]
pub struct PyTables {
    tables: Arc<Tables>,
}

impl From<Tables> for PyTables {
    fn from(tables: Tables) -> Self {
        Self {
            tables: Arc::new(tables),
        }
    }
}

impl PyTables {
    /// The number of tables.
    pub fn count(&self) -> usize {
        self.tables.len()
    }

    fn names(&self) -> Vec<String> {
        self.tables
            .iter()
            .map(|table| table.name().to_string())
            .collect()
    }
}

#[pymethods]
impl PyTables {
    /// The table names, in spec order.
    fn keys(&self) -> Vec<String> {
        self.names()
    }

    fn __len__(&self) -> usize {
        self.count()
    }

    fn __contains__(&self, name: &str) -> bool {
        self.tables.get(name).is_some()
    }

    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyIterator>> {
        PyTuple::new(py, self.names())?.try_iter()
    }

    fn __getitem__(&self, name: &str) -> PyResult<PyTable> {
        match self.tables.iter().position(|table| table.name() == name) {
            Some(index) => Ok(PyTable {
                tables: Arc::clone(&self.tables),
                index,
            }),
            None => Err(PyKeyError::new_err(format!(
                "there is no table \"{name}\"; the tables are: {}",
                self.names().join(", ")
            ))),
        }
    }

    /// Every table as text: title, header and one line per row.
    fn render(&self, py: Python<'_>) -> String {
        py.detach(|| {
            let mut out = String::new();
            for table in self.tables.iter() {
                render_table(table, &mut out);
            }
            out
        })
    }

    fn __repr__(&self) -> String {
        let parts = self
            .tables
            .iter()
            .map(|table| format!("{}: {} rows", table.name(), table.len()))
            .collect::<Vec<_>>()
            .join(", ");
        format!("Tables({parts})")
    }
}

/// One projected table. Shares the tables it belongs to.
#[pyclass(name = "Table", module = "oxedi835", frozen)]
pub struct PyTable {
    tables: Arc<Tables>,
    index: usize,
}

impl PyTable {
    fn table(&self) -> PyResult<&Table> {
        self.tables
            .iter()
            .nth(self.index)
            .ok_or_else(|| PyRuntimeError::new_err(format!("table #{} is gone", self.index)))
    }
}

#[pymethods]
impl PyTable {
    /// The table name.
    #[getter]
    fn name(&self) -> PyResult<String> {
        Ok(self.table()?.name().to_string())
    }

    /// The column names, in order.
    #[getter]
    fn columns(&self) -> PyResult<Vec<String>> {
        Ok(self
            .table()?
            .columns()
            .iter()
            .map(|(name, _)| name.clone())
            .collect())
    }

    fn __len__(&self) -> PyResult<usize> {
        Ok(self.table()?.len())
    }

    /// The table as text: title, header and one line per row.
    fn render(&self, py: Python<'_>) -> PyResult<String> {
        let table = self.table()?;
        Ok(py.detach(|| {
            let mut out = String::new();
            render_table(table, &mut out);
            out
        }))
    }

    fn __repr__(&self) -> PyResult<String> {
        let table = self.table()?;
        Ok(format!(
            "Table(name='{}', rows={}, columns={})",
            table.name(),
            table.len(),
            table.columns().len()
        ))
    }
}
