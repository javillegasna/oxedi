//! `Diagnostic`: one finding about a file's data, as Python attributes.

use edi835_core::{Diagnostic, SnipLevel};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyList};

/// One finding about the data of a file. A value, never raised.
#[pyclass(name = "Diagnostic", module = "oxedi835", frozen)]
pub struct PyDiagnostic {
    inner: Diagnostic,
}

impl From<Diagnostic> for PyDiagnostic {
    fn from(inner: Diagnostic) -> Self {
        Self { inner }
    }
}

fn position(value: Option<usize>) -> String {
    value.map_or_else(|| "None".to_string(), |value| value.to_string())
}

#[pymethods]
impl PyDiagnostic {
    /// The SNIP level of the rule: 1, 2 or 3.
    #[getter]
    fn level(&self) -> u8 {
        match self.inner.level {
            SnipLevel::L1 => 1,
            SnipLevel::L2 => 2,
            SnipLevel::L3 => 3,
        }
    }

    /// The name of the rule that failed, e.g. `RequiredElementMissing`.
    #[getter]
    fn kind(&self) -> &'static str {
        self.inner.rule.kind()
    }

    /// The rule that failed, as a sentence with its values.
    #[getter]
    fn rule(&self) -> String {
        self.inner.rule.to_string()
    }

    /// The index of the segment at fault; `None` at the end of the stream.
    #[getter]
    fn segment(&self) -> Option<usize> {
        self.inner.segment
    }

    /// The 1-based element position, when the finding has one.
    #[getter]
    fn element(&self) -> Option<usize> {
        self.inner.element
    }

    /// The 1-based component position, when the finding has one.
    #[getter]
    fn component(&self) -> Option<usize> {
        self.inner.component
    }

    /// The open loops, outermost first, joined by `/`; empty at the root.
    #[getter]
    fn path(&self) -> String {
        self.inner
            .path
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("/")
    }

    /// The offending value as it appears in the file.
    #[getter]
    fn datum<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.datum)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!(
            "Diagnostic(level={}, kind='{}', segment={}, element={}, component={})",
            self.level(),
            self.kind(),
            position(self.inner.segment),
            position(self.inner.element),
            position(self.inner.component)
        )
    }
}

/// The diagnostics as a Python list of `Diagnostic`.
pub fn to_list(py: Python<'_>, diagnostics: Vec<Diagnostic>) -> PyResult<Bound<'_, PyList>> {
    let list = PyList::empty(py);
    for diagnostic in diagnostics {
        list.append(Py::new(py, PyDiagnostic::from(diagnostic))?)?;
    }
    Ok(list)
}
