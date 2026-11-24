//! `run_kit`: runs a conformance kit over bytes the caller read, as typed objects.

use std::collections::BTreeMap;

use oxedi_core::Kit;
use oxedi_core::conformance::{self, CaseReport, KitReport, Outcome};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyMapping, PyString};
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyfunction, gen_stub_pymethods};

use crate::spec::SpecError;
use crate::validate::PyValid;

/// A `SpecError` with the core's text of a kit that does not read or does not fit its stack.
fn kit_error(error: impl std::fmt::Display) -> PyErr {
    SpecError::new_err(error.to_string())
}

/// What running one case gave.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(name = "CaseReport", module = "oxedi", frozen)]
pub struct PyCaseReport {
    name: String,
    passed: bool,
    text: String,
}

impl PyCaseReport {
    fn from_core(case: &CaseReport) -> PyCaseReport {
        PyCaseReport {
            name: case.name.clone(),
            passed: case.outcome == Outcome::Passed,
            text: case.to_string(),
        }
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

    /// The case and how it ended, as one line: where each output first
    /// differs, or why the case could not run.
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
            .map(|case| Py::new(py, PyCaseReport::from_core(case)))
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
