//! `Spec`: the loop structure, element definitions and tables, as data.

use std::sync::{Arc, OnceLock};

use edi835_core::Spec;
use pyo3::create_exception;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyString};

create_exception!(
    oxedi835,
    SpecError,
    PyValueError,
    "A spec that cannot be loaded or patched; the message names the rule, the loop and key, and the datum."
);

static BUILTIN: OnceLock<Arc<Spec>> = OnceLock::new();

/// The built-in spec, loaded once per process.
pub fn builtin() -> Arc<Spec> {
    BUILTIN
        .get_or_init(|| Arc::new(Spec::builtin_835()))
        .clone()
}

/// The spec a call uses: the one given, or the built-in.
pub fn or_builtin(spec: Option<&Bound<'_, PySpec>>) -> Arc<Spec> {
    spec.map_or_else(builtin, |spec| spec.get().inner.clone())
}

/// The loop structure, element definitions and tables of a file format.
#[pyclass(name = "Spec", module = "oxedi835", frozen)]
pub struct PySpec {
    pub inner: Arc<Spec>,
}

#[pymethods]
impl PySpec {
    /// The built-in 835 spec.
    #[staticmethod]
    fn builtin() -> PySpec {
        PySpec { inner: builtin() }
    }

    /// The spec described by `json`; raises `SpecError` with the core message
    /// when it is not valid.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<PySpec> {
        let spec = Spec::from_json(json).map_err(|err| SpecError::new_err(err.to_string()))?;
        Ok(PySpec {
            inner: Arc::new(spec),
        })
    }

    /// A new spec with `patch` (a dict or JSON text) merged over this one;
    /// raises `SpecError` when the result is not valid.
    fn patch(&self, py: Python<'_>, patch: &Bound<'_, PyAny>) -> PyResult<PySpec> {
        let text = if let Ok(text) = patch.cast::<PyString>() {
            text.to_cow()?.into_owned()
        } else if patch.is_instance_of::<PyDict>() {
            py.import("json")?
                .call_method1("dumps", (patch,))?
                .extract::<String>()?
        } else {
            return Err(PyTypeError::new_err(format!(
                "Spec.patch takes a dict or a JSON string, not {}",
                patch.get_type().name()?
            )));
        };
        let spec = self
            .inner
            .merge_patch(&text)
            .map_err(|err| SpecError::new_err(err.to_string()))?;
        Ok(PySpec {
            inner: Arc::new(spec),
        })
    }

    /// The spec as JSON text.
    fn to_json(&self) -> String {
        self.inner.to_json()
    }

    /// The loop names, in spec order.
    fn loops(&self) -> Vec<String> {
        self.inner
            .loops()
            .iter()
            .map(|def| def.name.clone())
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "Spec(name='{}', loops={}, tables={})",
            self.inner.name(),
            self.inner.loops().len(),
            self.inner.tables().len()
        )
    }
}
