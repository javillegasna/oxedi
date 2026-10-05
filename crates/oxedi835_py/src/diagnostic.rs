//! `Diagnostic`: one finding about a file's data, as Python attributes.

use edi835_core::{Diagnostic, Rule, SnipLevel};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyInt, PyList};

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

    /// Who reported the finding: `"oxedi835"` for the parser's own rules, the
    /// validator's name for an external finding.
    #[getter]
    fn origin(&self) -> &str {
        match &self.inner.rule {
            Rule::External { origin, .. } => origin,
            _ => "oxedi835",
        }
    }

    /// The external validator's own code for the finding; `None` for the
    /// parser's own rules and for external findings without one.
    #[getter]
    fn code(&self) -> Option<&str> {
        match &self.inner.rule {
            Rule::External { code, .. } => code.as_deref(),
            _ => None,
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
            "Diagnostic(level={}, origin='{}', kind='{}', segment={}, element={}, component={})",
            self.level(),
            self.origin(),
            self.kind(),
            position(self.inner.segment),
            position(self.inner.element),
            position(self.inner.component)
        )
    }
}

/// A position argument as an index. Any Python integer outside `usize` is a
/// `ValueError` naming the argument and the value; other types keep Python's
/// own `TypeError`.
fn position_arg(name: &str, value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<usize>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(None);
    };
    match value.extract::<usize>() {
        Ok(index) => Ok(Some(index)),
        Err(_) if value.is_instance_of::<PyInt>() => Err(PyValueError::new_err(format!(
            "{name} must be a non-negative integer, got {}",
            value.str()?
        ))),
        Err(err) => Err(err),
    }
}

/// Builds a `Diagnostic` for a finding reported by an external validator.
///
/// Internal: adapters to external validators call it; it is not part of the
/// public API. The finding has no loop path; `level` must be 1, 2 or 3.
#[pyfunction]
#[pyo3(
    name = "_external_diagnostic",
    signature = (origin, message, level, code=None, segment=None, element=None, component=None, datum=b"".to_vec())
)]
#[allow(clippy::too_many_arguments)]
pub fn external_diagnostic(
    origin: String,
    message: String,
    level: &Bound<'_, PyAny>,
    code: Option<String>,
    segment: Option<&Bound<'_, PyAny>>,
    element: Option<&Bound<'_, PyAny>>,
    component: Option<&Bound<'_, PyAny>>,
    datum: Vec<u8>,
) -> PyResult<PyDiagnostic> {
    let segment = position_arg("segment", segment)?;
    let element = position_arg("element", element)?;
    let component = position_arg("component", component)?;
    let level = match level.extract::<i64>() {
        Ok(level) => level,
        Err(_) if level.is_instance_of::<PyInt>() => {
            return Err(PyValueError::new_err(format!(
                "level must be 1, 2 or 3, got {}",
                level.str()?
            )));
        }
        Err(err) => return Err(err),
    };
    let level = match level {
        1 => SnipLevel::L1,
        2 => SnipLevel::L2,
        3 => SnipLevel::L3,
        _ => {
            return Err(PyValueError::new_err(format!(
                "level must be 1, 2 or 3, got {level}"
            )));
        }
    };
    let rule = Rule::External {
        origin,
        code,
        message,
        level,
    };
    Ok(PyDiagnostic::from(Diagnostic::new(
        rule,
        segment,
        element,
        component,
        Vec::new(),
        datum,
    )))
}

/// The diagnostics as a Python list of `Diagnostic`.
pub fn to_list(py: Python<'_>, diagnostics: Vec<Diagnostic>) -> PyResult<Bound<'_, PyList>> {
    let list = PyList::empty(py);
    for diagnostic in diagnostics {
        list.append(Py::new(py, PyDiagnostic::from(diagnostic))?)?;
    }
    Ok(list)
}
