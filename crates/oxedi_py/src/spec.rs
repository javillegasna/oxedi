//! `Spec`: the loop structure, element definitions and tables, as data.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ops::Deref;
use std::sync::Arc;

use oxedi_core::{
    Builtin, LoadedSpec, Segment, Spec, SpecError as CoreSpecError, Validation, Version,
};
use pyo3::exceptions::{PyKeyError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyCapsule, PyDict, PyString};

use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use pyo3_stub_gen::{PyStubType, TypeInfo};

use crate::arrow;
use crate::element_source::{self, PyElementSource, PyGroupSource};
use crate::json_arg::PatchArg;
use crate::tables::extra;
use crate::validate::Pool;

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

/// Where a spec's model is read from: kept beside a spec loaded or patched
/// from JSON, or read from a built-in when asked for, so a built-in carries
/// no model while only its compiled spec is used.
#[derive(Clone)]
enum ModelOf {
    Kept(Arc<LoadedSpec>),
    Builtin(&'static Builtin),
}

/// A spec as the binding holds it: the compiled spec the engine runs and
/// where its model is read from. Cloning it copies two pointers.
#[derive(Clone)]
pub struct HeldSpec {
    spec: Arc<Spec>,
    model: ModelOf,
}

impl HeldSpec {
    fn builtin(builtin: &'static Builtin) -> HeldSpec {
        HeldSpec {
            spec: Arc::clone(&builtin.spec),
            model: ModelOf::Builtin(builtin),
        }
    }

    pub(crate) fn kept(loaded: LoadedSpec) -> HeldSpec {
        HeldSpec {
            spec: Arc::clone(&loaded.spec),
            model: ModelOf::Kept(Arc::new(loaded)),
        }
    }

    /// The compiled spec, shared.
    pub fn spec(&self) -> &Arc<Spec> {
        &self.spec
    }

    /// The spec with its model, for `call` to patch or write: the kept one, or
    /// the built-in's model read now. A read that fails is a `SpecError`
    /// naming `call`.
    fn loaded(&self, call: &str) -> PyResult<Cow<'_, LoadedSpec>> {
        match &self.model {
            ModelOf::Kept(loaded) => Ok(Cow::Borrowed(&**loaded)),
            ModelOf::Builtin(builtin) => builtin
                .loaded()
                .map(Cow::Owned)
                .map_err(|err| SpecError::new_err(format!("{call}: {err}"))),
        }
    }
}

impl Deref for HeldSpec {
    type Target = Spec;

    fn deref(&self) -> &Spec {
        &self.spec
    }
}

fn spec_error(err: CoreSpecError) -> PyErr {
    SpecError::new_err(err.to_string())
}

/// The spec given to a call, which it uses as is.
pub fn given(spec: Option<&Bound<'_, PySpec>>) -> Option<HeldSpec> {
    spec.map(|spec| spec.get().inner.clone())
}

/// The spec a call uses: the one given, or else the built-in of the version
/// the segments declare, and the default built-in when they declare none of
/// theirs. Reading stops once the version is settled.
pub fn given_or_selected<'s>(
    given: Option<HeldSpec>,
    segments: impl IntoIterator<Item = Segment<'s>>,
) -> HeldSpec {
    given.unwrap_or_else(|| HeldSpec::builtin(Builtin::select(segments)))
}

/// An X12 release a built-in spec covers.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(name = "Version", module = "oxedi", eq, frozen, hash, from_py_object)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PyVersion(Version);

impl From<Version> for PyVersion {
    fn from(version: Version) -> PyVersion {
        PyVersion(version)
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PyVersion {
    /// Release 5010.
    #[classattr]
    #[pyo3(name = "V5010")]
    fn v5010() -> PyVersion {
        Version::V5010.into()
    }

    /// Release 4010.
    #[classattr]
    #[pyo3(name = "V4010")]
    fn v4010() -> PyVersion {
        Version::V4010.into()
    }

    /// The release number as text.
    #[getter]
    fn text(&self) -> &'static str {
        self.0.text()
    }

    fn __str__(&self) -> &'static str {
        self.text()
    }

    fn __repr__(&self) -> String {
        format!("Version.V{}", self.0.text())
    }

    /// Every version a built-in spec covers, the default first.
    #[staticmethod]
    fn all() -> Vec<PyVersion> {
        Version::ALL.into_iter().map(PyVersion::from).collect()
    }
}

/// A version argument: a `Version`, its text, or anything else (kept as its
/// repr and type name, so the call that received it can refuse it).
pub enum VersionArg {
    Version(Version),
    Text(String),
    Other { shown: String, kind: String },
}

impl VersionArg {
    /// The version `Spec.builtin` uses when none is given.
    const DEFAULT: VersionArg = VersionArg::Version(Builtin::DEFAULT_VERSION);

    /// The built-in of `transaction` the argument names, for the `call` that
    /// received it (e.g. `Spec.builtin`): `TypeError` for an argument that is
    /// neither a `Version` nor text, `ValueError` for a version no built-in
    /// covers.
    fn resolve(&self, call: &str, transaction: &str) -> PyResult<&'static Builtin> {
        let found = match self {
            VersionArg::Version(version) => Builtin::find(transaction, *version),
            VersionArg::Text(text) => Builtin::find_text(transaction, text),
            VersionArg::Other { shown, kind } => {
                return Err(PyTypeError::new_err(format!(
                    "{call}(version={shown}): version must be an oxedi.Version or its text, not {kind}"
                )));
            }
        };
        found.map_err(|error| {
            PyValueError::new_err(format!("{call}(version={:?}): {error}", error.version))
        })
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for VersionArg {
    type Error = PyErr;

    fn extract(object: Borrowed<'a, 'py, PyAny>) -> PyResult<VersionArg> {
        if let Ok(version) = object.extract::<PyVersion>() {
            return Ok(VersionArg::Version(version.0));
        }
        if let Ok(text) = object.extract::<String>() {
            return Ok(VersionArg::Text(text));
        }
        let shown = object.repr().map_or_else(
            |_| "an object whose repr failed".to_owned(),
            |text| text.to_string(),
        );
        let kind = object
            .get_type()
            .qualname()
            .map_or_else(|_| "an unknown type".to_owned(), |name| name.to_string());
        Ok(VersionArg::Other { shown, kind })
    }
}

impl<'py> IntoPyObject<'py> for VersionArg {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self {
            VersionArg::Version(version) => {
                Ok(PyVersion::from(version).into_pyobject(py)?.into_any())
            }
            VersionArg::Text(text) => Ok(PyString::new(py, &text).into_any()),
            // This conversion only renders the default, a `Version`; this arm is never reached.
            VersionArg::Other { shown, .. } => Ok(PyString::new(py, &shown).into_any()),
        }
    }
}

impl PyStubType for VersionArg {
    fn type_output() -> TypeInfo {
        PyVersion::type_output()
    }

    fn type_input() -> TypeInfo {
        PyVersion::type_input()
    }
}

/// The loop structure, element definitions and tables of a file format.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(name = "Spec", module = "oxedi", frozen)]
pub struct PySpec {
    pub inner: HeldSpec,
}

impl From<HeldSpec> for PySpec {
    fn from(inner: HeldSpec) -> PySpec {
        PySpec { inner }
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PySpec {
    /// The version of the default built-in spec.
    #[classattr]
    #[pyo3(name = "DEFAULT_VERSION")]
    fn default_version() -> PyVersion {
        Builtin::DEFAULT_VERSION.into()
    }

    /// The built-in spec of `version`; it defaults to `Spec.DEFAULT_VERSION`.
    /// Raises `ValueError` for a version no built-in covers.
    #[staticmethod]
    #[pyo3(signature = (version = VersionArg::DEFAULT))]
    fn builtin(version: VersionArg) -> PyResult<PySpec> {
        let found = version.resolve("Spec.builtin", Builtin::fallback().transaction.as_str())?;
        Ok(HeldSpec::builtin(found).into())
    }

    /// The spec described by `json`; raises `SpecError` with the core message
    /// when it is not valid.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<PySpec> {
        LoadedSpec::from_json(json)
            .map(HeldSpec::kept)
            .map(PySpec::from)
            .map_err(spec_error)
    }

    /// The spec of the stack whose top package is the text `top`, with the
    /// package texts of `pool` as its bases; raises `SpecError` with the
    /// core's message when the stack does not load.
    #[staticmethod]
    #[pyo3(signature = (top, pool = Pool::default()), text_signature = "(top, pool=())")]
    fn from_packages(top: &str, pool: Pool) -> PyResult<PySpec> {
        match oxedi_core::validate(top, &pool.texts()) {
            Validation::Yes(valid) => Ok(HeldSpec::kept(valid.loaded).into()),
            Validation::No(no) => Err(SpecError::new_err(no.to_string())),
        }
    }

    /// The package texts the library ships: the 5010 stack, then the 4010
    /// editions. They are the pool of `Spec.from_packages` and `validate_stack`.
    #[staticmethod]
    fn builtin_packages() -> Vec<&'static str> {
        Builtin::packages().to_vec()
    }

    /// A new spec with `patch` merged over this one; `patch` is JSON text or a
    /// JSON object (a dict of JSON values); raises `SpecError` when the
    /// result is not valid.
    fn patch(
        &self,
        // A JSON merge patch: JSON text, or an object of JSON values.
        #[gen_stub(override_type(type_repr = "builtins.str | _json.JsonObject", imports = ("builtins", "oxedi._json")))]
        patch: PatchArg,
    ) -> PyResult<PySpec> {
        let json = patch.into_json()?;
        self.inner
            .loaded("Spec.patch")?
            .merge_patch(&json)
            .map(|patched| HeldSpec::kept(patched).into())
            .map_err(spec_error)
    }

    /// The spec's model as canonical JSON text: no key equal to its default,
    /// no `null`, keys of maps in name order. Raises `SpecError` when the
    /// model cannot be read or written.
    fn to_json(&self) -> PyResult<String> {
        self.inner.loaded("Spec.to_json")?.to_json().map_err(|err| {
            SpecError::new_err(format!(
                "Spec.to_json: the model cannot be written as JSON: {err}"
            ))
        })
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

    /// Where column `column` of table `table` reads its value, as the spec
    /// compiled it. Raises `KeyError` for a table or column the spec does not
    /// declare, and `ValueError` for a column that reads a segment index or an
    /// element of a repeated group, which has no single element.
    fn column_source(&self, table: &str, column: &str) -> PyResult<PyElementSource> {
        element_source::resolve(&self.inner, table, column)
    }

    /// The table names, in name order.
    fn tables(&self) -> Vec<String> {
        self.inner
            .tables()
            .iter()
            .map(|table| table.name.clone())
            .collect()
    }

    /// The id of the segment that opens loop `name`, such as `b"CLP"`, without
    /// the element conditions the trigger may add. Raises `KeyError` for a loop
    /// the spec does not declare.
    fn trigger_segment<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyBytes>> {
        let Some(id) = self.inner.loop_id(name) else {
            let known = self
                .loops()
                .iter()
                .map(|loop_name| format!("{loop_name:?}"))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(PyKeyError::new_err(format!(
                "Spec.trigger_segment({name:?}): loop {name:?} is not a loop of the spec; its loops are {known}"
            )));
        };
        Ok(PyBytes::new(py, &self.inner.get(id).trigger.segment))
    }

    /// Every column of table `table` that reads one fixed element, by name, in
    /// name order; a column holding a segment index or reading a repeated group
    /// is not listed. Raises `KeyError` for a table the spec does not declare.
    fn element_sources(&self, table: &str) -> PyResult<BTreeMap<String, PyElementSource>> {
        element_source::element_sources(&self.inner, table)
    }

    /// Every column of table `table` that reads an element of each repeated
    /// group, by name, in name order; empty for a table without `repeat`.
    /// Raises `KeyError` for a table the spec does not declare.
    fn group_sources(&self, table: &str) -> PyResult<BTreeMap<String, PyGroupSource>> {
        element_source::group_sources(&self.inner, table)
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
