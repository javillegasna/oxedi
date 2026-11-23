//! `validate`: whether a stack of package texts loads, as typed objects.

use oxedi_core::{Capability, Invalid, Reasons, Valid, Validation};
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyfunction, gen_stub_pymethods};

use crate::spec::{HeldSpec, PySpec};

/// How many characters of the answer `repr(Invalid)` shows.
const REPR_CHARS: usize = 60;

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

/// A stack of packages that loads.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(name = "Valid", module = "oxedi", frozen)]
pub struct PyValid {
    id: String,
    name: String,
    packages: Vec<String>,
    write: Py<PyCapability>,
    mask: Py<PyCapability>,
    changes: Vec<String>,
    text: String,
    spec: HeldSpec,
}

impl PyValid {
    fn new(py: Python<'_>, valid: Valid) -> PyResult<PyValid> {
        let text = valid.to_string();
        let changes = valid.change_lines();
        Ok(PyValid {
            id: valid.id.to_string(),
            name: valid.name,
            packages: valid
                .members
                .iter()
                .map(|member| member.id.to_string())
                .collect(),
            write: Py::new(py, PyCapability::from_core(&valid.write))?,
            mask: Py::new(py, PyCapability::from_core(&valid.mask))?,
            changes,
            text,
            spec: HeldSpec::kept(valid.loaded),
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

    /// What the editions and patches of the stack changed, in stack order,
    /// one line each with the package that made it.
    #[getter]
    fn changes(&self) -> Vec<String> {
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

    fn __repr__(&self) -> String {
        format!("Valid(id='{}', name='{}')", self.id, self.name)
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

    fn __repr__(&self) -> String {
        let shown: String = self.text.chars().take(REPR_CHARS).collect();
        let more = if self.text.chars().count() > REPR_CHARS {
            "..."
        } else {
            ""
        };
        format!("Invalid({shown}{more})")
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
#[pyo3(signature = (top, pool = Vec::new()), text_signature = "(top, pool=[])")]
#[gen_stub(override_return_type(type_repr = "Valid | Invalid", imports = ()))]
pub fn validate(py: Python<'_>, top: &str, pool: Vec<String>) -> PyResult<Py<PyAny>> {
    let texts: Vec<&str> = pool.iter().map(String::as_str).collect();
    match oxedi_core::validate(top, &texts) {
        Validation::Yes(valid) => Ok(Py::new(py, PyValid::new(py, *valid)?)?.into_any()),
        Validation::No(no) => Ok(Py::new(py, invalid(&no))?.into_any()),
    }
}
