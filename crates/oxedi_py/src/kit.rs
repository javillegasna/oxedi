//! `run_kit`: runs a conformance kit over bytes the caller read, as typed objects.

use std::collections::BTreeMap;

use oxedi_core::Kit;
use oxedi_core::conformance::{self, CaseFault, CaseReport, Difference, KitReport, Line, Outcome};
use oxedi_core::spec_load::KitOutput;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyMapping, PyString};
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyfunction, gen_stub_pymethods};

use crate::spec::SpecError;
use crate::validate::PyValid;

/// A `SpecError` with the core's text of a kit that does not read or does not fit its stack.
fn kit_error(error: impl std::fmt::Display) -> PyErr {
    SpecError::new_err(error.to_string())
}

/// The output of a run a case can expect: one member per output of the core.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(
    name = "KitOutput",
    module = "oxedi",
    eq,
    frozen,
    hash,
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PyKitOutput(Output);

/// The core's outputs, which Python compares and hashes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Output {
    Tables,
    Diagnostics,
}

impl From<KitOutput> for PyKitOutput {
    fn from(output: KitOutput) -> PyKitOutput {
        PyKitOutput(match output {
            KitOutput::Tables => Output::Tables,
            KitOutput::Diagnostics => Output::Diagnostics,
        })
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PyKitOutput {
    /// The tables.
    #[classattr]
    #[pyo3(name = "TABLES")]
    fn tables() -> PyKitOutput {
        PyKitOutput(Output::Tables)
    }

    /// The diagnostics.
    #[classattr]
    #[pyo3(name = "DIAGNOSTICS")]
    fn diagnostics() -> PyKitOutput {
        PyKitOutput(Output::Diagnostics)
    }

    fn __repr__(&self) -> String {
        match self.0 {
            Output::Tables => "KitOutput.TABLES".to_owned(),
            Output::Diagnostics => "KitOutput.DIAGNOSTICS".to_owned(),
        }
    }
}

/// The text of an output ended before the line a difference names.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(
    name = "KitEnd",
    module = "oxedi",
    eq,
    frozen,
    hash,
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PyKitEnd;

#[gen_stub_pymethods]
#[pymethods]
impl PyKitEnd {
    fn __repr__(&self) -> &'static str {
        "KitEnd()"
    }
}

/// The first line where an output differs from the text the kit expects.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(name = "KitDifference", module = "oxedi", frozen)]
pub struct PyKitDifference {
    output: PyKitOutput,
    file: String,
    line: usize,
    expected: Py<PyAny>,
    actual: Py<PyAny>,
    text: String,
}

impl PyKitDifference {
    fn from_core(py: Python<'_>, difference: &Difference) -> PyResult<PyKitDifference> {
        let side = |line: &Line| match line {
            Line::Text(text) => Ok(PyString::new(py, text).into_any().unbind()),
            Line::End => Py::new(py, PyKitEnd).map(Py::into_any),
        };
        Ok(PyKitDifference {
            output: PyKitOutput::from(difference.output),
            file: difference.file.clone(),
            line: difference.at.line,
            expected: side(&difference.at.expected)?,
            actual: side(&difference.at.actual)?,
            text: difference.to_string(),
        })
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PyKitDifference {
    /// The output that differs.
    #[getter]
    fn output(&self) -> PyKitOutput {
        self.output
    }

    /// The file that holds the expected text, as the kit writes its path.
    #[getter]
    fn file(&self) -> String {
        self.file.clone()
    }

    /// The 1-based number of the first line that differs.
    #[getter]
    fn line(&self) -> usize {
        self.line
    }

    /// What the expected text has there: the line without its line break, or
    /// `KitEnd` when it ended before.
    #[getter]
    #[gen_stub(override_return_type(type_repr = "str | KitEnd", imports = ()))]
    fn expected(&self, py: Python<'_>) -> Py<PyAny> {
        self.expected.clone_ref(py)
    }

    /// What the run's output has there, as `expected`.
    #[getter]
    #[gen_stub(override_return_type(type_repr = "str | KitEnd", imports = ()))]
    fn actual(&self, py: Python<'_>) -> Py<PyAny> {
        self.actual.clone_ref(py)
    }

    fn __str__(&self) -> String {
        self.text.clone()
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "KitDifference(output={}, file={}, line={})",
            self.output.__repr__(),
            PyString::new(py, &self.file).repr()?,
            self.line
        ))
    }
}

/// A case whose every expected output matches.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(
    name = "KitPassed",
    module = "oxedi",
    eq,
    frozen,
    hash,
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PyKitPassed;

#[gen_stub_pymethods]
#[pymethods]
impl PyKitPassed {
    fn __repr__(&self) -> &'static str {
        "KitPassed()"
    }
}

/// A case where some expected output differs, one difference per output.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(name = "KitDiffers", module = "oxedi", frozen)]
pub struct PyKitDiffers {
    differences: Vec<Py<PyKitDifference>>,
}

impl PyKitDiffers {
    fn from_core(py: Python<'_>, found: &[Difference]) -> PyResult<PyKitDiffers> {
        let differences = found
            .iter()
            .map(|difference| Py::new(py, PyKitDifference::from_core(py, difference)?))
            .collect::<PyResult<_>>()?;
        Ok(PyKitDiffers { differences })
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PyKitDiffers {
    /// The differing outputs, in output order.
    #[getter]
    fn differences(&self, py: Python<'_>) -> Vec<Py<PyKitDifference>> {
        self.differences.iter().map(|d| d.clone_ref(py)).collect()
    }
}

/// A case that could not run: the file it names was not given, is not text or
/// is not an X12 document.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(
    name = "KitBroken",
    module = "oxedi",
    eq,
    frozen,
    hash,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PyKitBroken {
    path: String,
    message: String,
}

impl From<&CaseFault> for PyKitBroken {
    fn from(fault: &CaseFault) -> PyKitBroken {
        PyKitBroken {
            path: fault.path().to_owned(),
            message: fault.to_string(),
        }
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PyKitBroken {
    /// The file the fault names, as the kit writes its path.
    #[getter]
    fn path(&self) -> String {
        self.path.clone()
    }

    /// What failed and what to do.
    #[getter]
    fn message(&self) -> String {
        self.message.clone()
    }

    fn __str__(&self) -> String {
        self.message.clone()
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "KitBroken(path={})",
            PyString::new(py, &self.path).repr()?
        ))
    }
}

/// What running one case gave.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(name = "CaseReport", module = "oxedi", frozen)]
pub struct PyCaseReport {
    name: String,
    outcome: Py<PyAny>,
    passed: bool,
    text: String,
}

impl PyCaseReport {
    fn from_core(py: Python<'_>, case: &CaseReport) -> PyResult<PyCaseReport> {
        let outcome = match &case.outcome {
            Outcome::Passed => Py::new(py, PyKitPassed)?.into_any(),
            Outcome::Broken(fault) => Py::new(py, PyKitBroken::from(fault))?.into_any(),
            Outcome::Differs(found) => Py::new(py, PyKitDiffers::from_core(py, found)?)?.into_any(),
        };
        Ok(PyCaseReport {
            name: case.name.clone(),
            outcome,
            passed: case.outcome == Outcome::Passed,
            text: case.to_string(),
        })
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PyCaseReport {
    /// The case's name.
    #[getter]
    fn name(&self) -> String {
        self.name.clone()
    }

    /// Whether every expected output matches.
    #[getter]
    fn passed(&self) -> bool {
        self.passed
    }

    /// How the case ended.
    #[getter]
    #[gen_stub(override_return_type(type_repr = "KitPassed | KitDiffers | KitBroken", imports = ()))]
    fn outcome(&self, py: Python<'_>) -> Py<PyAny> {
        self.outcome.clone_ref(py)
    }

    fn __str__(&self) -> String {
        self.text.clone()
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "CaseReport(name={}, passed={})",
            PyString::new(py, &self.name).repr()?,
            if self.passed { "True" } else { "False" }
        ))
    }
}

/// What running a kit gave: one case report per case, in name order.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(name = "KitReport", module = "oxedi", frozen)]
pub struct PyKitReport {
    package: String,
    cases: Vec<Py<PyCaseReport>>,
    passed: bool,
    text: String,
}

impl PyKitReport {
    fn from_core(py: Python<'_>, report: &KitReport) -> PyResult<PyKitReport> {
        let cases = report
            .cases
            .iter()
            .map(|case| Py::new(py, PyCaseReport::from_core(py, case)?))
            .collect::<PyResult<_>>()?;
        Ok(PyKitReport {
            package: report.package.to_string(),
            cases,
            passed: report.passed(),
            text: report.to_string(),
        })
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PyKitReport {
    /// The id of the top package of the stack the kit checks.
    #[getter]
    fn package(&self) -> String {
        self.package.clone()
    }

    /// Whether every case passed.
    #[getter]
    fn passed(&self) -> bool {
        self.passed
    }

    /// Each case's report, in name order.
    #[getter]
    fn cases(&self, py: Python<'_>) -> Vec<Py<PyCaseReport>> {
        self.cases.iter().map(|case| case.clone_ref(py)).collect()
    }

    fn __str__(&self) -> String {
        self.text.clone()
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "KitReport(package={}, passed={})",
            PyString::new(py, &self.package).repr()?,
            if self.passed { "True" } else { "False" }
        ))
    }
}

/// The paths a kit manifest names, sorted: the files `run_kit` needs. They are
/// relative to the manifest's folder and may hold `..`, so the caller keeps them under a root it
/// trusts. Raises `SpecError` with the core's message when the manifest does not read.
#[gen_stub_pyfunction(module = "oxedi._core")]
#[pyfunction]
pub fn kit_files(manifest: &str) -> PyResult<Vec<String>> {
    let kit = Kit::from_json(manifest).map_err(kit_error)?;
    Ok(kit.files().into_iter().map(str::to_owned).collect())
}

/// Runs the kit `manifest` (the manifest's text) on the validated stack
/// `stack`. `files` maps each path `kit_files` lists, as the kit writes it, to
/// its bytes: nothing is opened, and the caller keeps the paths under a root it
/// trusts. Raises `SpecError` with the core's message when the manifest does
/// not read or checks another package than the stack's top.
#[gen_stub_pyfunction(module = "oxedi._core")]
#[pyfunction]
pub fn run_kit(
    py: Python<'_>,
    manifest: &str,
    stack: &PyValid,
    #[gen_stub(override_type(
        type_repr = "typing.Mapping[builtins.str, builtins.bytes]",
        imports = ("typing", "builtins")
    ))]
    files: &Bound<'_, PyMapping>,
) -> PyResult<PyKitReport> {
    let kit = Kit::from_json(manifest).map_err(kit_error)?;
    let items: Vec<(String, Bound<'_, PyBytes>)> = files.items()?.extract()?;
    let given: BTreeMap<&str, &[u8]> = items
        .iter()
        .map(|(path, bytes)| (path.as_str(), bytes.as_bytes()))
        .collect();
    let core = stack.core();
    let report = py
        .detach(|| conformance::run_kit(&kit, core, &given))
        .map_err(kit_error)?;
    PyKitReport::from_core(py, &report)
}
