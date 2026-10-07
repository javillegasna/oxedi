//! `parse`: one pass over a whole file, with the GIL released.

use oxedi_core::Processor;
use std::sync::Arc;

use pyo3::buffer::PyBuffer;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString};
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyfunction, gen_stub_pymethods};

use crate::diagnostic;
use crate::document::{self, PyDelimiters, PyDocument};
use crate::native::{self, Role};
use crate::spec::{self, PySpec};
use crate::tables::PyTables;

/// Copies the input exactly once into memory Rust owns. Any object with the
/// buffer protocol is accepted (`bytes`, `bytearray`, `memoryview`, `mmap`).
pub fn copy_input(function: &str, data: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    if data.is_instance_of::<PyString>() {
        return Err(PyTypeError::new_err(
            "the input must be bytes or another buffer, not str: open the file in binary mode or encode the text",
        ));
    }
    match PyBuffer::<u8>::get(data) {
        Ok(buffer) => buffer.to_vec(data.py()),
        Err(_) => Err(PyTypeError::new_err(format!(
            "{function}: argument data must be a buffer of unsigned bytes, format 'B', 'b' or 'c'; found {}",
            found(data)
        ))),
    }
}

/// What the object is, for the refusal: its buffer format when it has one,
/// else its type name.
fn found(data: &Bound<'_, PyAny>) -> String {
    let format = data
        .py()
        .import("builtins")
        .and_then(|builtins| builtins.getattr("memoryview"))
        .and_then(|view| view.call1((data,)))
        .and_then(|view| view.getattr("format"))
        .and_then(|format| format.repr())
        .map(|format| format.to_string());
    match format {
        Ok(format) => format!("format {format}"),
        Err(_) => format!(
            "type {}",
            data.get_type()
                .name()
                .map_or_else(|_| "?".to_string(), |name| name.to_string())
        ),
    }
}

/// What one parse produced.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(name = "Result", module = "oxedi", frozen)]
pub struct PyParseResult {
    document: Py<PyDocument>,
    tables: Py<PyTables>,
    diagnostics: Py<PyList>,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyParseResult {
    /// The file, held losslessly.
    #[getter]
    fn document(&self, py: Python<'_>) -> Py<PyDocument> {
        self.document.clone_ref(py)
    }

    /// The projected tables, by name.
    #[getter]
    fn tables(&self, py: Python<'_>) -> Py<PyTables> {
        self.tables.clone_ref(py)
    }

    /// The spec that projected the tables: the one given, or the built-in
    /// selected from the file's version.
    #[getter]
    fn spec(&self, py: Python<'_>) -> Option<PySpec> {
        self.tables.bind(py).get().spec().map(|inner| PySpec {
            inner: Arc::clone(inner),
        })
    }

    /// Every diagnostic, in stream order.
    #[getter]
    #[gen_stub(override_return_type(type_repr = "builtins.list[Diagnostic]", imports = ("builtins",)))]
    fn diagnostics(&self, py: Python<'_>) -> Py<PyList> {
        self.diagnostics.clone_ref(py)
    }

    /// The rows of `claims`.
    fn count_claims(&self, py: Python<'_>) -> PyResult<usize> {
        native::count_claims(self.tables.bind(py).get().tables())
    }

    /// The distinct non-null values of `claims.patient_id`. Ids are text:
    /// `0123` and `123` are two patients, where edi-835-parser reads both as
    /// the number 123 and counts one.
    fn count_patients(&self, py: Python<'_>) -> PyResult<usize> {
        native::count_patients(self.tables.bind(py).get().tables())
    }

    /// The sum of `payments.total_payment_amount`, as a `Decimal` at the
    /// column's scale.
    #[gen_stub(override_return_type(type_repr = "decimal.Decimal", imports = ("decimal",)))]
    fn sum_payments<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let sum = native::sum_payments(self.tables.bind(py).get().tables())?;
        py.import("decimal")?.getattr("Decimal")?.call1((sum,))
    }

    /// The payer organization from the `payer_*` columns of `payments`, or
    /// `None` when the file has none.
    #[getter]
    #[gen_stub(override_return_type(type_repr = "builtins.dict[builtins.str, builtins.str | None] | None", imports = ("builtins",)))]
    fn payer<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyDict>>> {
        organization(py, self.tables.bind(py).get().tables(), Role::Payer)
    }

    /// The payee organization from the `payee_*` columns of `payments`, or
    /// `None` when the file has none.
    #[getter]
    #[gen_stub(override_return_type(type_repr = "builtins.dict[builtins.str, builtins.str | None] | None", imports = ("builtins",)))]
    fn payee<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyDict>>> {
        organization(py, self.tables.bind(py).get().tables(), Role::Payee)
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        format!(
            "Result(segments={}, tables={}, diagnostics={})",
            self.document.bind(py).get().inner.len(),
            self.tables.bind(py).get().count(),
            self.diagnostics.bind(py).len()
        )
    }
}

/// The organization as a dict, or `None`.
fn organization<'py>(
    py: Python<'py>,
    tables: &oxedi_core::Tables,
    role: Role,
) -> PyResult<Option<Bound<'py, PyDict>>> {
    let Some(fields) = native::organization(tables, role)? else {
        return Ok(None);
    };
    let dict = PyDict::new(py);
    for (key, value) in fields {
        dict.set_item(key, value)?;
    }
    Ok(Some(dict))
}

/// Parses a whole file: indexes every segment, runs the loop engine, the
/// envelope checker and the projector, and returns the document, the
/// tables and every diagnostic. Without `spec`, the built-in spec of the
/// version the file declares is used, else the default one.
#[gen_stub_pyfunction]
#[pyfunction]
#[pyo3(signature = (data, spec = None, delimiters = None))]
pub fn parse(
    py: Python<'_>,
    #[gen_stub(override_type(type_repr = "typing_extensions.Buffer", imports = ("typing_extensions",)))]
    data: &Bound<'_, PyAny>,
    spec: Option<&Bound<'_, PySpec>>,
    delimiters: Option<&Bound<'_, PyDelimiters>>,
) -> PyResult<PyParseResult> {
    let bytes = copy_input("parse", data)?;
    let given = spec::given(spec);
    let delimiters = delimiters.map(|d| d.get().inner);
    let (document, tables, diagnostics) = py.detach(|| -> PyResult<_> {
        let document = document::index(bytes, delimiters)?;
        let spec = spec::given_or_selected(given, document.segments());
        let (tables, diagnostics) = Processor::run(&spec, &document);
        Ok((document, PyTables::projected(tables, spec), diagnostics))
    })?;
    let diagnostics = diagnostic::to_list(py, diagnostics)?;
    Ok(PyParseResult {
        document: Py::new(py, PyDocument { inner: document })?,
        tables: Py::new(py, tables)?,
        diagnostics: diagnostics.unbind(),
    })
}
