//! `validate_stack`: whether a stack of package texts loads, as typed objects.

use oxedi_core::spec_load::{Change, ChangeKind};
use oxedi_core::{Capability, Invalid, Reasons, Valid, Validation};
use pyo3::prelude::*;
use pyo3::types::{PyString, PyTuple};
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyfunction, gen_stub_pymethods};
use pyo3_stub_gen::{PyStubType, TypeInfo};

use crate::spec::{HeldSpec, PySpec};

/// How many characters of the answer `repr(Invalid)` shows.
const REPR_CHARS: usize = 60;

/// The package texts a stack builds on. It reads any sequence of `str` (a `str`
/// itself is refused, as it would read as characters) and its default is the
/// empty tuple, which the stub and the signature show as `()`.
#[derive(Debug, Clone, Default)]
pub struct Pool(pub Vec<String>);

impl<'a, 'py> FromPyObject<'a, 'py> for Pool {
    type Error = PyErr;

    fn extract(object: Borrowed<'a, 'py, PyAny>) -> PyResult<Pool> {
        object.extract::<Vec<String>>().map(Pool)
    }
}

impl<'py> IntoPyObject<'py> for Pool {
    type Target = PyTuple;
    type Output = Bound<'py, PyTuple>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.0)
    }
}

impl PyStubType for Pool {
    fn type_input() -> TypeInfo {
        <Vec<String> as PyStubType>::type_input()
    }

    fn type_output() -> TypeInfo {
        <Vec<String> as PyStubType>::type_output()
    }
}

impl Pool {
    /// The texts, borrowed.
    pub fn texts(&self) -> Vec<&str> {
        self.0.iter().map(String::as_str).collect()
    }
}

/// What a stack that loads can do beyond reading: whether it can, and when it
/// cannot, one line per reason, each saying what to change.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(name = "Capability", module = "oxedi", frozen)]
pub struct PyCapability {
    can: bool,
    reasons: Vec<String>,
}

impl PyCapability {
    fn from_core<R: Reasons>(capability: &Capability<R>) -> PyCapability {
        PyCapability {
            can: capability.can(),
            reasons: capability.reasons(),
        }
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PyCapability {
    /// Whether the stack can.
    #[getter]
    fn can(&self) -> bool {
        self.can
    }

    /// Why it cannot, one line each; empty when it can.
    #[getter]
    fn reasons(&self) -> Vec<String> {
        self.reasons.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "Capability(can={}, reasons={})",
            if self.can { "True" } else { "False" },
            self.reasons.len()
        )
    }
}

/// How a change left a value, compared with the package it builds on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    Narrowed,
    Widened,
    Changed,
    Removed,
    Added,
}

impl From<&ChangeKind> for Kind {
    fn from(kind: &ChangeKind) -> Kind {
        match kind {
            ChangeKind::Narrowed { .. } => Kind::Narrowed,
            ChangeKind::Widened { .. } => Kind::Widened,
            ChangeKind::Changed { .. } => Kind::Changed,
            ChangeKind::Removed => Kind::Removed,
            ChangeKind::Added { .. } => Kind::Added,
        }
    }
}

/// How a change left a value, compared with the package it builds on: one
/// member per kind of the core's change.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(
    name = "ChangeKind",
    module = "oxedi",
    eq,
    frozen,
    hash,
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PyChangeKind(Kind);

impl From<&ChangeKind> for PyChangeKind {
    fn from(kind: &ChangeKind) -> PyChangeKind {
        PyChangeKind(Kind::from(kind))
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PyChangeKind {
    /// It allows less than before.
    #[classattr]
    #[pyo3(name = "NARROWED")]
    fn narrowed() -> PyChangeKind {
        PyChangeKind(Kind::Narrowed)
    }

    /// It allows more than before.
    #[classattr]
    #[pyo3(name = "WIDENED")]
    fn widened() -> PyChangeKind {
        PyChangeKind(Kind::Widened)
    }

    /// It is neither narrower nor wider: a name, a type, a structure.
    #[classattr]
    #[pyo3(name = "CHANGED")]
    fn changed() -> PyChangeKind {
        PyChangeKind(Kind::Changed)
    }

    /// It is gone.
    #[classattr]
    #[pyo3(name = "REMOVED")]
    fn removed() -> PyChangeKind {
        PyChangeKind(Kind::Removed)
    }

    /// It is new.
    #[classattr]
    #[pyo3(name = "ADDED")]
    fn added() -> PyChangeKind {
        PyChangeKind(Kind::Added)
    }

    fn __repr__(&self) -> String {
        format!("ChangeKind.{}", self.member())
    }
}

impl PyChangeKind {
    /// The member's name as Python spells it.
    fn member(self) -> &'static str {
        match self.0 {
            Kind::Narrowed => "NARROWED",
            Kind::Widened => "WIDENED",
            Kind::Changed => "CHANGED",
            Kind::Removed => "REMOVED",
            Kind::Added => "ADDED",
        }
    }
}

/// One difference between a package and the stack it builds on: the package
/// that made it, where, and how.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(
    name = "Change",
    module = "oxedi",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PyChange {
    package: String,
    path: String,
    kind: PyChangeKind,
    line: String,
}

impl PyChange {
    /// `line` is the core's text of the change with its package.
    fn from_core(change: &Change, line: String) -> PyChange {
        PyChange {
            package: change.package.to_string(),
            path: change.path.clone(),
            kind: PyChangeKind::from(&change.kind),
            line,
        }
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PyChange {
    /// The id of the package that made the change.
    #[getter]
    fn package(&self) -> String {
        self.package.clone()
    }

    /// Where, from the spec's root, a dot between the names.
    #[getter]
    fn path(&self) -> String {
        self.path.clone()
    }

    /// How the value changed.
    #[getter]
    fn kind(&self) -> PyChangeKind {
        self.kind
    }

    /// The change and its package, as one line.
    fn __str__(&self) -> String {
        self.line.clone()
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "Change(package={}, path={}, kind={})",
            PyString::new(py, &self.package).repr()?,
            PyString::new(py, &self.path).repr()?,
            self.kind.__repr__()
        ))
    }
}

/// A stack of packages that loads.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(name = "Valid", module = "oxedi", frozen)]
pub struct PyValid {
    id: String,
    name: String,
    packages: Vec<String>,
    write: Py<PyCapability>,
    mask: Py<PyCapability>,
    changes: Vec<PyChange>,
    text: String,
    spec: HeldSpec,
    core: Valid,
}

impl PyValid {
    /// The core's answer, which the kit runner takes.
    pub(crate) fn core(&self) -> &Valid {
        &self.core
    }

    fn new(py: Python<'_>, valid: Valid) -> PyResult<PyValid> {
        let text = valid.to_string();
        let changes = valid
            .changes
            .iter()
            .zip(valid.change_lines())
            .map(|(change, line)| PyChange::from_core(change, line))
            .collect();
        Ok(PyValid {
            id: valid.id().to_string(),
            name: valid.name().to_owned(),
            packages: valid
                .members()
                .map(|member| member.id.to_string())
                .collect(),
            write: Py::new(py, PyCapability::from_core(&valid.write))?,
            mask: Py::new(py, PyCapability::from_core(&valid.mask))?,
            changes,
            text,
            spec: HeldSpec::kept(valid.loaded.clone()),
            core: valid,
        })
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PyValid {
    /// The top package's id.
    #[getter]
    fn id(&self) -> String {
        self.id.clone()
    }

    /// The spec's name.
    #[getter]
    fn name(&self) -> String {
        self.name.clone()
    }

    /// The id of every package of the stack, bases first, the top last.
    #[getter]
    fn packages(&self) -> Vec<String> {
        self.packages.clone()
    }

    /// Whether the spec's tables can be written back into a file, and why not.
    #[getter]
    fn write(&self, py: Python<'_>) -> Py<PyCapability> {
        self.write.clone_ref(py)
    }

    /// Whether the spec can mask patient data, and why not.
    #[getter]
    fn mask(&self, py: Python<'_>) -> Py<PyCapability> {
        self.mask.clone_ref(py)
    }

    /// What the editions and patches of the stack changed, in stack order.
    #[getter]
    fn changes(&self) -> Vec<PyChange> {
        self.changes.clone()
    }

    /// The spec the stack loads as.
    #[getter]
    fn spec(&self) -> PySpec {
        self.spec.clone().into()
    }

    fn __str__(&self) -> String {
        self.text.clone()
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "Valid(id={}, name={})",
            PyString::new(py, &self.id).repr()?,
            PyString::new(py, &self.name).repr()?
        ))
    }
}

/// A stack of packages that does not load.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(name = "Invalid", module = "oxedi", frozen)]
pub struct PyInvalid {
    text: String,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyInvalid {
    fn __str__(&self) -> String {
        self.text.clone()
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let mut shown: String = self.text.chars().take(REPR_CHARS).collect();
        if self.text.chars().count() > REPR_CHARS {
            shown.push_str("...");
        }
        Ok(format!("Invalid({})", PyString::new(py, &shown).repr()?))
    }
}

/// The text of `Invalid` for a stack the core refuses.
fn invalid(invalid: &Invalid) -> PyInvalid {
    PyInvalid {
        text: invalid.to_string(),
    }
}

/// Whether the stack whose top package is `top`, with the packages of `pool`
/// as its bases, loads: a `Valid` with what it holds and can do, or an
/// `Invalid` with the reason. `pool` is a list of package texts.
#[gen_stub_pyfunction(module = "oxedi._core")]
#[pyfunction]
#[pyo3(signature = (top, pool = Pool::default()), text_signature = "(top, pool=())")]
#[gen_stub(override_return_type(type_repr = "Valid | Invalid", imports = ()))]
pub fn validate_stack(py: Python<'_>, top: &str, pool: Pool) -> PyResult<Py<PyAny>> {
    match oxedi_core::validate(top, &pool.texts()) {
        Validation::Yes(valid) => Ok(Py::new(py, PyValid::new(py, *valid)?)?.into_any()),
        Validation::No(no) => Ok(Py::new(py, invalid(&no))?.into_any()),
    }
}
