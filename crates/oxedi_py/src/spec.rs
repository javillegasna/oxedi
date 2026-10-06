//! `Spec`: the loop structure, element definitions and tables, as data.

use std::sync::{Arc, OnceLock};

use edi835_core::{Segment, Spec};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyString};
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};

native_exception!(
    SpecError,
    PyValueError,
    "A spec that cannot be loaded or patched; the message names the rule, the loop and key, and the datum."
);

static BUILTIN: OnceLock<Arc<Spec>> = OnceLock::new();
static BUILTIN_4010: OnceLock<Arc<Spec>> = OnceLock::new();

/// The built-in spec, loaded once per process.
pub fn builtin() -> Arc<Spec> {
    BUILTIN
        .get_or_init(|| Arc::new(Spec::builtin_835()))
        .clone()
}

/// The built-in 4010 spec, loaded once per process.
fn builtin_4010() -> Arc<Spec> {
    BUILTIN_4010
        .get_or_init(|| Arc::new(Spec::builtin_835_4010()))
        .clone()
}

/// The spec given to a call, which it uses as is.
pub fn given(spec: Option<&Bound<'_, PySpec>>) -> Option<Arc<Spec>> {
    spec.map(|spec| spec.get().inner.clone())
}

/// The spec a call uses: the one given, or else the built-in of the version
/// the segments declare, and the default built-in when they declare none of
/// theirs. Reading stops once the version is settled.
pub fn given_or_selected<'s>(
    given: Option<Arc<Spec>>,
    segments: impl IntoIterator<Item = Segment<'s>>,
) -> Arc<Spec> {
    if let Some(spec) = given {
        return spec;
    }
    let (default, other) = (builtin(), builtin_4010());
    let chosen = Spec::select(&[&default, &other], &default, segments);
    if std::ptr::eq(chosen, &*other) {
        other
    } else {
        default
    }
}

/// The loop structure, element definitions and tables of a file format.
#[gen_stub_pyclass(module = "oxedi835._core")]
#[pyclass(name = "Spec", module = "oxedi835", frozen)]
pub struct PySpec {
    pub inner: Arc<Spec>,
}

#[gen_stub_pymethods]
#[pymethods]
impl PySpec {
    /// The built-in 835 spec of `version`, `"5010"` (the default) or
    /// `"4010"`; raises `ValueError` for any other version.
    #[staticmethod]
    #[pyo3(signature = (version = "5010"))]
    fn builtin(
        // Described as written so the stub carries the default value.
        #[gen_stub(override_type(type_repr = "builtins.str", imports = ("builtins",)))]
        version: &str,
    ) -> PyResult<PySpec> {
        let inner = match version {
            "5010" => builtin(),
            "4010" => builtin_4010(),
            other => {
                return Err(PyValueError::new_err(format!(
                    "Spec.builtin(version={other:?}): no built-in spec for that version; the built-ins are \"5010\" and \"4010\""
                )));
            }
        };
        Ok(PySpec { inner })
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
    fn patch(
        &self,
        py: Python<'_>,
        // A JSON merge patch: the dict's values are any JSON value.
        #[gen_stub(override_type(type_repr = "builtins.str | builtins.dict[builtins.str, typing.Any]", imports = ("builtins", "typing")))]
        patch: &Bound<'_, PyAny>,
    ) -> PyResult<PySpec> {
        let text = if let Ok(text) = patch.cast::<PyString>() {
            text.to_cow()?.into_owned()
        } else if patch.is_instance_of::<PyDict>() {
            py.import("json")?
                .call_method1("dumps", (patch,))
                .and_then(|text| text.extract::<String>())
                .map_err(|err| {
                    PyTypeError::new_err(format!(
                        "Spec.patch: the dict cannot be written as JSON: {}",
                        err.value(py)
                    ))
                })?
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
