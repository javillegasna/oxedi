//! `Spec`: the loop structure, element definitions and tables, as data.

use std::sync::Arc;

use oxedi_core::{Builtin, Segment, Spec};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyCapsule, PyDict, PyString};

use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};

use crate::arrow;
use crate::tables::extra;

native_exception!(
    SpecError,
    PyValueError,
    "A spec that cannot be loaded or patched; the message names the rule, the loop and key, and the datum."
);

/// One table's schema, offered to `pyarrow.schema` through the Arrow
/// PyCapsule interface; it is not part of the module's public names.
#[pyclass(frozen)]
struct SchemaExport {
    name: String,
    schema: arrow_schema::Schema,
}

#[pymethods]
impl SchemaExport {
    fn __arrow_c_schema__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyCapsule>> {
        let schema = arrow::export_schema(&self.name, &self.schema)?;
        PyCapsule::new_with_value(py, schema, c"arrow_schema")
    }
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
    Arc::clone(&Builtin::select(segments).spec)
}

/// The loop structure, element definitions and tables of a file format.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(name = "Spec", module = "oxedi", frozen)]
pub struct PySpec {
    pub inner: Arc<Spec>,
}

#[gen_stub_pymethods]
#[pymethods]
impl PySpec {
    /// The version of the default built-in spec.
    #[classattr]
    const DEFAULT_VERSION: &'static str = Builtin::DEFAULT_VERSION;

    /// The built-in spec of `version`; it defaults to `Spec.DEFAULT_VERSION`.
    /// Raises `ValueError` for a version no built-in covers.
    #[staticmethod]
    #[pyo3(signature = (version = Builtin::DEFAULT_VERSION))]
    fn builtin(version: &str) -> PyResult<PySpec> {
        let found = Builtin::find(Builtin::fallback().transaction, version).map_err(|error| {
            PyValueError::new_err(format!("Spec.builtin(version={version:?}): {error}"))
        })?;
        Ok(PySpec {
            inner: Arc::clone(&found.spec),
        })
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

    /// The schema of every table of the spec, by name, in table order: one
    /// `pyarrow.Schema` per table, with the column names and Arrow types the
    /// tables of a parse with this spec have. A reference column (such as
    /// `claim` in `services`) holds the `row` of a row in another table, and
    /// its field metadata names that table under the key `oxedi.parent`.
    /// Needs pyarrow.
    #[gen_stub(override_return_type(type_repr = "builtins.dict[builtins.str, pyarrow.Schema]", imports = ("builtins", "pyarrow")))]
    fn schema<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let make = extra(py, "Spec", "schema", "pyarrow", "pandas")?.getattr("schema")?;
        let schemas = PyDict::new(py);
        for (name, schema) in arrow::spec_schemas(&self.inner)? {
            let export = Bound::new(
                py,
                SchemaExport {
                    name: name.clone(),
                    schema,
                },
            )?;
            schemas.set_item(name, make.call1((export,))?)?;
        }
        Ok(schemas)
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
