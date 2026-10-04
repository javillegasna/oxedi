//! `Tables` and `Table`: the projected tables, readable as text or as Arrow.

use std::sync::Arc;

use edi835_core::{Table, Tables};
use pyo3::exceptions::{PyImportError, PyKeyError, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::types::{PyCapsule, PyDict, PyIterator, PyTuple};

use crate::arrow;

/// Imports `module`, or raises an `ImportError` that names the method, the
/// module and the extra that provides it, with the original error as cause.
fn extra<'py>(
    py: Python<'py>,
    owner: &str,
    method: &str,
    module: &str,
    extra: &str,
) -> PyResult<Bound<'py, PyModule>> {
    py.import(module).map_err(|original| {
        let error = PyImportError::new_err(format!(
            "{owner}.{method} needs {module}, which is not installed; install it with: pip install \"oxedi835[{extra}]\""
        ));
        error.set_cause(py, Some(original));
        error
    })
}

/// A table as a polars frame; `owner` names the class in the error.
fn to_polars<'py>(
    py: Python<'py>,
    owner: &str,
    table: Bound<'py, PyTable>,
) -> PyResult<Bound<'py, PyAny>> {
    extra(py, owner, "to_polars", "polars", "polars")?
        .getattr("DataFrame")?
        .call1((table,))
}

/// A table as a pandas frame, through Arrow; `owner` names the class in the error.
fn to_pandas<'py>(
    py: Python<'py>,
    owner: &str,
    table: Bound<'py, PyTable>,
) -> PyResult<Bound<'py, PyAny>> {
    extra(py, owner, "to_pandas", "pandas", "pandas")?;
    extra(py, owner, "to_pandas", "pyarrow", "pandas")?
        .getattr("table")?
        .call1((table,))?
        .call_method0("to_pandas")
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

    /// The tables themselves.
    pub fn tables(&self) -> &Arc<Tables> {
        &self.tables
    }

    fn table_at(&self, index: usize) -> PyTable {
        PyTable {
            tables: Arc::clone(&self.tables),
            index,
        }
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

    fn __contains__(&self, name: &Bound<'_, PyAny>) -> bool {
        name.extract::<&str>()
            .is_ok_and(|name| self.tables.get(name).is_some())
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
        py.detach(|| self.tables.to_string())
    }

    /// Every table as a polars `DataFrame`, by name, in table order.
    fn to_polars<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyDict>> {
        let frames = PyDict::new(slf.py());
        for index in 0..slf.get().count() {
            let table = Bound::new(slf.py(), slf.get().table_at(index))?;
            let name = table.get().name()?;
            frames.set_item(name, to_polars(slf.py(), "Tables", table)?)?;
        }
        Ok(frames)
    }

    /// Every table as a pandas `DataFrame`, by name, in table order.
    fn to_pandas<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyDict>> {
        let frames = PyDict::new(slf.py());
        for index in 0..slf.get().count() {
            let table = Bound::new(slf.py(), slf.get().table_at(index))?;
            let name = table.get().name()?;
            frames.set_item(name, to_pandas(slf.py(), "Tables", table)?)?;
        }
        Ok(frames)
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

    /// The table as a polars `DataFrame`, through the Arrow capsule.
    fn to_polars<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        to_polars(slf.py(), "Table", slf.clone())
    }

    /// The table as a pandas `DataFrame`, through `pyarrow`.
    fn to_pandas<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        to_pandas(slf.py(), "Table", slf.clone())
    }

    /// The table as text: title, header and one line per row.
    fn render(&self, py: Python<'_>) -> PyResult<String> {
        let table = self.table()?;
        Ok(py.detach(|| table.to_string()))
    }

    /// Exports the table as an Arrow stream of one record batch, sharing
    /// the column buffers. `requested_schema` is ignored, as the protocol
    /// allows: the table keeps its own types.
    #[pyo3(signature = (requested_schema = None))]
    fn __arrow_c_stream__<'py>(
        &self,
        py: Python<'py>,
        requested_schema: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyCapsule>> {
        let _ = requested_schema;
        let stream = py.detach(|| arrow::stream(&self.tables, self.index))?;
        PyCapsule::new_with_value(py, stream, c"arrow_array_stream")
    }

    /// Exports the table as one Arrow struct array and its schema, sharing
    /// the column buffers. `requested_schema` is ignored, as the protocol
    /// allows.
    #[pyo3(signature = (requested_schema = None))]
    fn __arrow_c_array__<'py>(
        &self,
        py: Python<'py>,
        requested_schema: Option<Bound<'py, PyAny>>,
    ) -> PyResult<(Bound<'py, PyCapsule>, Bound<'py, PyCapsule>)> {
        let _ = requested_schema;
        let (array, schema) = py.detach(|| arrow::array(&self.tables, self.index))?;
        Ok((
            PyCapsule::new_with_value(py, schema, c"arrow_schema")?,
            PyCapsule::new_with_value(py, array, c"arrow_array")?,
        ))
    }

    /// Exports the table's schema.
    fn __arrow_c_schema__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyCapsule>> {
        let schema = arrow::schema(&self.tables, self.index)?;
        PyCapsule::new_with_value(py, schema, c"arrow_schema")
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
