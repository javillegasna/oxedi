//! `Tables` and `Table`: the projected tables, readable as text or as Arrow.

use std::sync::Arc;

use oxedi_core::{Spec, Table, Tables};
use pyo3::exceptions::{PyImportError, PyKeyError, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::types::{PyCapsule, PyDict, PyIterator, PyTuple};
use pyo3_stub_gen::derive::{gen_methods_from_python, gen_stub_pyclass, gen_stub_pymethods};

use crate::arrow;
use crate::spec::PySpec;

/// Imports `module`, or raises an `ImportError` that names the method, the
/// module and the extra that provides it, with the original error as cause.
pub(crate) fn extra<'py>(
    py: Python<'py>,
    owner: &str,
    method: &str,
    module: &str,
    extra: &str,
) -> PyResult<Bound<'py, PyModule>> {
    py.import(module).map_err(|original| {
        let error = PyImportError::new_err(format!(
            "{owner}.{method} needs {module}, which is not installed; install it with: pip install \"oxedi[{extra}]\""
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
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(name = "Tables", module = "oxedi", frozen, mapping)]
pub struct PyTables {
    tables: Arc<Tables>,
    /// The spec that projected them, which writing them back uses by default.
    spec: Arc<Spec>,
}

impl PyTables {
    /// Tables projected with `spec`.
    pub fn projected(tables: Tables, spec: Arc<Spec>) -> Self {
        Self {
            tables: Arc::new(tables),
            spec,
        }
    }

    /// The spec that projected the tables.
    pub fn spec(&self) -> &Arc<Spec> {
        &self.spec
    }

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

#[gen_stub_pymethods]
#[pymethods]
impl PyTables {
    /// The spec that projected the tables, which `write` uses by default.
    #[getter(spec)]
    fn spec_of_parse(&self) -> PySpec {
        PySpec {
            inner: Arc::clone(&self.spec),
        }
    }

    /// The table names, in spec order.
    fn keys(&self) -> Vec<String> {
        self.names()
    }

    fn __len__(&self) -> usize {
        self.count()
    }

    #[gen_stub(skip)]
    fn __contains__(&self, name: &Bound<'_, PyAny>) -> bool {
        name.extract::<&str>()
            .is_ok_and(|name| self.tables.get(name).is_some())
    }

    #[gen_stub(override_return_type(type_repr = "collections.abc.Iterator[builtins.str]", imports = ("builtins", "collections.abc")))]
    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyIterator>> {
        PyTuple::new(py, self.names())?.try_iter()
    }

    #[gen_stub(skip)]
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
    #[gen_stub(override_return_type(type_repr = "builtins.dict[builtins.str, polars.DataFrame]", imports = ("builtins", "polars")))]
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
    #[gen_stub(override_return_type(type_repr = "builtins.dict[builtins.str, pandas.DataFrame]", imports = ("builtins", "pandas")))]
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

// Slot methods take their argument by position only.
pyo3_stub_gen::inventory::submit! {
    gen_methods_from_python! {
        r#"
        class PyTables:
            def __contains__(self, name: builtins.object, /) -> builtins.bool: ...
            def __getitem__(self, name: builtins.str, /) -> Table: ...
        "#
    }
}

/// One projected table. Shares the tables it belongs to.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(name = "Table", module = "oxedi", frozen)]
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

#[gen_stub_pymethods]
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
    #[gen_stub(override_return_type(type_repr = "polars.DataFrame", imports = ("polars",)))]
    fn to_polars<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        to_polars(slf.py(), "Table", slf.clone())
    }

    /// The table as a pandas `DataFrame`, through `pyarrow`.
    #[gen_stub(override_return_type(type_repr = "pandas.DataFrame", imports = ("pandas",)))]
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
    #[gen_stub(override_return_type(type_repr = "typing_extensions.CapsuleType", imports = ("typing_extensions",)))]
    fn __arrow_c_stream__<'py>(
        &self,
        py: Python<'py>,
        #[gen_stub(override_type(type_repr = "builtins.object | None", imports = ("builtins",)))]
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
    #[gen_stub(override_return_type(type_repr = "builtins.tuple[typing_extensions.CapsuleType, typing_extensions.CapsuleType]", imports = ("builtins", "typing_extensions")))]
    fn __arrow_c_array__<'py>(
        &self,
        py: Python<'py>,
        #[gen_stub(override_type(type_repr = "builtins.object | None", imports = ("builtins",)))]
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
    #[gen_stub(override_return_type(type_repr = "typing_extensions.CapsuleType", imports = ("typing_extensions",)))]
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
