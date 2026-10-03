//! `parse`: one pass over a whole file, with the GIL released.

use edi835_core::Processor;
use pyo3::buffer::PyBuffer;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyString};

use crate::diagnostic;
use crate::document::{self, PyDelimiters, PyDocument};
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
#[pyclass(name = "Result", module = "oxedi835", frozen)]
pub struct PyParseResult {
    document: Py<PyDocument>,
    tables: Py<PyTables>,
    diagnostics: Py<PyList>,
}

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

    /// Every diagnostic, in stream order.
    #[getter]
    fn diagnostics(&self, py: Python<'_>) -> Py<PyList> {
        self.diagnostics.clone_ref(py)
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

/// Parses a whole file: indexes every segment, runs the loop engine, the
/// envelope checker and the projector, and returns the document, the
/// tables and every diagnostic.
#[pyfunction]
#[pyo3(signature = (data, spec = None, delimiters = None))]
pub fn parse(
    py: Python<'_>,
    data: &Bound<'_, PyAny>,
    spec: Option<&Bound<'_, PySpec>>,
    delimiters: Option<&Bound<'_, PyDelimiters>>,
) -> PyResult<PyParseResult> {
    let bytes = copy_input("parse", data)?;
    let spec = spec::or_builtin(spec);
    let delimiters = delimiters.map(|d| d.get().inner);
    let (document, tables, diagnostics) = py.detach(|| -> PyResult<_> {
        let document = document::index(bytes, delimiters)?;
        let (tables, diagnostics) = Processor::run(&spec, &document);
        Ok((document, tables, diagnostics))
    })?;
    let diagnostics = diagnostic::to_list(py, diagnostics)?;
    Ok(PyParseResult {
        document: Py::new(py, PyDocument { inner: document })?,
        tables: Py::new(py, PyTables::from(tables))?,
        diagnostics: diagnostics.unbind(),
    })
}
