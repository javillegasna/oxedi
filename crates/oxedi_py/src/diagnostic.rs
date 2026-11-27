//! `Diagnostic`: a reading diagnostic as Python attributes, and the adapters
//! that build and mask an external validator's findings.

use oxedi_core::problem::Problem;
use oxedi_core::{Diagnostic, LoopRef, Rule, SnipLevel};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyInt, PyList};
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyfunction, gen_stub_pymethods};
use pyo3_stub_gen::type_info::{
    MethodInfo, MethodType, ParameterDefault, ParameterInfo, ParameterKind, PyMethodsInfo,
};
use pyo3_stub_gen::{PyStubType, TypeInfo};

use crate::document::PyDocument;
use crate::mask::PyMask;
use crate::spec::PySpec;

/// One diagnostic about the data of a file: the rule that failed, where and
/// the value. A value, never raised.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(name = "Diagnostic", module = "oxedi", frozen)]
pub struct PyDiagnostic {
    pub(crate) inner: Diagnostic,
}

impl From<Diagnostic> for PyDiagnostic {
    fn from(inner: Diagnostic) -> Self {
        Self { inner }
    }
}

fn position(value: Option<usize>) -> String {
    value.map_or_else(|| "None".to_string(), |value| value.to_string())
}

#[gen_stub_pymethods]
#[pymethods]
impl PyDiagnostic {
    /// The SNIP level of the rule: 1, 2 or 3.
    #[getter]
    fn level(&self) -> u8 {
        self.inner.level.number()
    }

    /// Who reported the diagnostic: `"oxedi"` for the parser's own rules,
    /// the validator's name for an external one.
    #[getter]
    fn origin(&self) -> &str {
        self.inner.origin()
    }

    /// The external validator's own code for the finding; `None` for the
    /// parser's own rules and for external findings without one.
    #[getter]
    fn code(&self) -> Option<&str> {
        self.inner.code()
    }

    /// The name of the rule that failed, e.g. `RequiredElementMissing`.
    #[getter]
    fn kind(&self) -> &'static str {
        Problem::kind(&self.inner)
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

    /// The 1-based element position, when the diagnostic has one.
    #[getter]
    fn element(&self) -> Option<usize> {
        self.inner.element
    }

    /// The 1-based component position, when the diagnostic has one.
    #[getter]
    fn component(&self) -> Option<usize> {
        self.inner.component
    }

    /// The open loops, outermost first, joined by `/`; empty at the root.
    #[getter]
    fn path(&self) -> String {
        self.inner.path_text()
    }

    /// The offending value as it appears in the file.
    #[getter]
    fn datum<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.datum)
    }

    #[staticmethod]
    #[pyo3(
        signature = (origin, message, level, code=None, segment=None, element=None, component=None, datum=b"".to_vec(), path=Vec::new()),
        text_signature = "(origin, message, level, code=None, segment=None, element=None, component=None, datum=b'', path=())"
    )]
    #[gen_stub(skip)]
    #[allow(clippy::too_many_arguments)]
    fn external(
        origin: &str,
        message: &str,
        level: &Bound<'_, PyAny>,
        code: Option<String>,
        segment: Option<&Bound<'_, PyAny>>,
        element: Option<&Bound<'_, PyAny>>,
        component: Option<&Bound<'_, PyAny>>,
        datum: Vec<u8>,
        path: Vec<(String, usize)>,
    ) -> PyResult<PyDiagnostic> {
        external_diagnostic(
            origin, message, level, code, segment, element, component, datum, path,
        )
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

/// The level of an external finding, read by the core's rule; else `ValueError`.
fn level_arg(level: &Bound<'_, PyAny>) -> PyResult<SnipLevel> {
    match level.extract::<i64>() {
        Ok(number) => SnipLevel::from_number(number)
            .map_err(|unknown| PyValueError::new_err(unknown.to_string())),
        Err(_) if level.is_instance_of::<PyInt>() => Err(PyValueError::new_err(format!(
            "level must be 1, 2 or 3, got {}",
            level.str()?
        ))),
        Err(err) => Err(err),
    }
}

#[allow(clippy::too_many_arguments)]
fn external_diagnostic(
    origin: impl Into<String>,
    message: impl Into<String>,
    level: &Bound<'_, PyAny>,
    code: Option<String>,
    segment: Option<&Bound<'_, PyAny>>,
    element: Option<&Bound<'_, PyAny>>,
    component: Option<&Bound<'_, PyAny>>,
    datum: Vec<u8>,
    path: impl IntoIterator<Item = (String, usize)>,
) -> PyResult<PyDiagnostic> {
    let segment = position_arg("segment", segment)?;
    let element = position_arg("element", element)?;
    let component = position_arg("component", component)?;
    let level = level_arg(level)?;
    let rule = Rule::External {
        origin: origin.into(),
        code,
        message: message.into(),
        level,
    };
    let path = path
        .into_iter()
        .map(|(name, ordinal)| LoopRef { name, ordinal })
        .collect();
    Ok(PyDiagnostic::from(Diagnostic::new(
        rule, segment, element, component, path, datum,
    )))
}

/// A parameter of `Diagnostic.external` with no default.
const fn required(name: &'static str, type_info: fn() -> TypeInfo) -> ParameterInfo {
    ParameterInfo {
        name,
        kind: ParameterKind::PositionalOrKeyword,
        type_info,
        default: ParameterDefault::None,
    }
}

/// A parameter of `Diagnostic.external` defaulting to `default`, written as
/// a Python literal.
const fn defaulted(
    name: &'static str,
    type_info: fn() -> TypeInfo,
    default: fn() -> String,
) -> ParameterInfo {
    ParameterInfo {
        name,
        kind: ParameterKind::PositionalOrKeyword,
        type_info,
        default: ParameterDefault::Expr {
            value: default,
            source_module: None,
        },
    }
}

fn none() -> String {
    "None".to_string()
}

// Described by hand: the derive macros cannot write the `datum` default
// (an empty byte string); mirrors `signature =` on `external`.
pyo3_stub_gen::inventory::submit! {
    PyMethodsInfo {
        struct_id: std::any::TypeId::of::<PyDiagnostic>,
        attrs: &[],
        getters: &[],
        setters: &[],
        methods: &[MethodInfo {
            name: "external",
            parameters: &[
                required("origin", <String as PyStubType>::type_input),
                required("message", <String as PyStubType>::type_input),
                required("level", <i64 as PyStubType>::type_input),
                defaulted("code", <Option<String> as PyStubType>::type_input, none),
                defaulted("segment", <Option<usize> as PyStubType>::type_input, none),
                defaulted("element", <Option<usize> as PyStubType>::type_input, none),
                defaulted("component", <Option<usize> as PyStubType>::type_input, none),
                defaulted("datum", <Py<PyBytes> as PyStubType>::type_input, || "b\"\"".to_string()),
                defaulted(
                    "path",
                    <Vec<(String, usize)> as PyStubType>::type_input,
                    || "()".to_string(),
                ),
            ],
            r#return: <PyDiagnostic as PyStubType>::type_output,
            doc: "Builds the diagnostic of a finding an external validator reported, for adapters to such validators.\n\n`level` is the SNIP level, 1, 2 or 3; `code`, `segment`, `element` and `component` are `None` when the finding names no code or place; `path` holds the open loops as `(name, ordinal)` pairs, outermost first, as `Document.loop_paths` returns them. Raises `ValueError` for a level or a position that is not valid.",
            r#type: MethodType::Static,
            is_async: false,
            deprecated: None,
            type_ignored: None,
            is_overload: false,
        }],
        file: file!(),
        line: line!(),
        column: column!(),
    }
}

/// The private form of `Mask.diagnostics`, which takes the mask first; internal.
#[gen_stub_pyfunction]
#[pyfunction]
#[pyo3(name = "_mask_findings", signature = (diagnostics, document, mask, spec, own_text=None))]
pub fn mask_findings(
    diagnostics: Vec<PyRef<'_, PyDiagnostic>>,
    document: &Bound<'_, PyDocument>,
    mask: &Bound<'_, PyMask>,
    spec: &Bound<'_, PySpec>,
    own_text: Option<Vec<bool>>,
) -> Vec<PyDiagnostic> {
    mask.get()
        .diagnostics(diagnostics, document, spec, own_text.unwrap_or_default())
}

pyo3_stub_gen::export_verbatim!("oxedi._core", "_mask_findings");

/// The diagnostics as a Python list of `Diagnostic`.
pub fn to_list(py: Python<'_>, diagnostics: Vec<Diagnostic>) -> PyResult<Bound<'_, PyList>> {
    let list = PyList::empty(py);
    for diagnostic in diagnostics {
        list.append(Py::new(py, PyDiagnostic::from(diagnostic))?)?;
    }
    Ok(list)
}
