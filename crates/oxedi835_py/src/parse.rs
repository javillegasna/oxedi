//! `parse`: one pass over a whole file, with the GIL released.

use pyo3::buffer::PyBuffer;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::PyString;

use crate::document::{self, PyDelimiters, PyDocument};

/// Copies the input exactly once into memory Rust owns. Any object with the
/// buffer protocol is accepted (`bytes`, `bytearray`, `memoryview`, `mmap`).
pub fn copy_input(data: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    if data.is_instance_of::<PyString>() {
        return Err(PyTypeError::new_err(
            "the input must be bytes or another buffer, not str: open the file in binary mode or encode the text",
        ));
    }
    let buffer = PyBuffer::<u8>::get(data)?;
    buffer.to_vec(data.py())
}

/// What one parse produced.
#[pyclass(name = "Result", module = "oxedi835", frozen)]
pub struct PyParseResult {
    document: Py<PyDocument>,
}

#[pymethods]
impl PyParseResult {
    /// The file, held losslessly.
    #[getter]
    fn document(&self, py: Python<'_>) -> Py<PyDocument> {
        self.document.clone_ref(py)
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        format!(
            "Result(segments={})",
            self.document.bind(py).get().inner.len()
        )
    }
}

/// Parses a whole file: copies the input once and indexes every segment.
#[pyfunction]
#[pyo3(signature = (data, *, delimiters = None))]
pub fn parse(
    py: Python<'_>,
    data: &Bound<'_, PyAny>,
    delimiters: Option<&Bound<'_, PyDelimiters>>,
) -> PyResult<PyParseResult> {
    let bytes = copy_input(data)?;
    let delimiters = delimiters.map(|d| d.get().inner);
    let document = py.detach(|| document::index(bytes, delimiters))?;
    Ok(PyParseResult {
        document: Py::new(py, PyDocument { inner: document })?,
    })
}
