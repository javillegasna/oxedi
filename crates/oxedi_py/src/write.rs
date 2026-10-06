//! `Envelope`, `WriteFinding` and `_write`: tables back into a file.
//!
//! `_write` converts its tables to the core's columns (the tables of a parse
//! are used as they are) and calls the core's writer with the GIL released.
//! Every refusal raises `oxedi.WriteError`, defined in the Python package
//! with the findings as an attribute; `oxedi.write` wraps `_write`.

use std::sync::Arc;

use oxedi_core::write::{Envelope, Finding, Origin, WriteError, write_with_findings};
use oxedi_core::{Spec, Tables};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyList, PyMapping, PyTuple};
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyfunction, gen_stub_pymethods};

use crate::diagnostic::PyDiagnostic;
use crate::document::PyDelimiters;
use crate::import;
use crate::spec::{self, PySpec};
use crate::tables::PyTables;

/// Days from 0001-01-01 (ordinal 1) to 1970-01-01.
const EPOCH_ORDINAL: i64 = 719_163;

/// Who sends and receives an interchange, when, its first control number
/// and its delimiters. The writer derives the rest of the envelope: counts,
/// later control numbers, codes and fixed widths.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(name = "Envelope", module = "oxedi", frozen)]
pub struct PyEnvelope {
    inner: Envelope,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyEnvelope {
    /// `date` (a `datetime.date`) and `time` (a `datetime.time` without a
    /// time zone, whole seconds) stamp the interchange and the group: the
    /// interchange header holds the date without its century and the time
    /// without its seconds, the group header keeps both. `delimiters`
    /// defaults to `*`, `:`, `~` and the repetition separator `^`;
    /// `line_break` adds a line break after each segment.
    #[new]
    #[pyo3(signature = (
        *,
        sender_id,
        receiver_id,
        date,
        time,
        sender_qualifier = "ZZ",
        receiver_qualifier = "ZZ",
        usage_indicator = "P",
        control_number = 1,
        application_sender = None,
        application_receiver = None,
        delimiters = None,
        line_break = false
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        sender_id: String,
        receiver_id: String,
        #[gen_stub(override_type(type_repr = "datetime.date", imports = ("datetime",)))]
        date: &Bound<'_, PyAny>,
        #[gen_stub(override_type(type_repr = "datetime.time", imports = ("datetime",)))]
        time: &Bound<'_, PyAny>,
        #[gen_stub(override_type(type_repr = "builtins.str", imports = ("builtins",)))]
        sender_qualifier: &str,
        #[gen_stub(override_type(type_repr = "builtins.str", imports = ("builtins",)))]
        receiver_qualifier: &str,
        #[gen_stub(override_type(type_repr = "builtins.str", imports = ("builtins",)))]
        usage_indicator: &str,
        #[gen_stub(override_type(type_repr = "builtins.int", imports = ("builtins",)))]
        control_number: u64,
        application_sender: Option<String>,
        application_receiver: Option<String>,
        delimiters: Option<&Bound<'_, PyDelimiters>>,
        #[gen_stub(override_type(type_repr = "builtins.bool", imports = ("builtins",)))]
        line_break: bool,
    ) -> PyResult<Self> {
        let datetime = date.py().import("datetime")?;
        if date.is_instance(&datetime.getattr("datetime")?)? {
            return Err(PyValueError::new_err(format!(
                "date {date} is a datetime; give a datetime.date (the envelope's time is its own field)"
            )));
        }
        if !time.getattr("tzinfo")?.is_none() {
            return Err(PyValueError::new_err(format!(
                "time {time} has a time zone; give a datetime.time without one, as the envelope writes local time"
            )));
        }
        let ordinal: i64 = date.call_method0("toordinal")?.extract()?;
        let days = i32::try_from(ordinal - EPOCH_ORDINAL)
            .map_err(|_| PyValueError::new_err(format!("date {date} is out of range")))?;
        let part = |name: &str| -> PyResult<i32> { time.getattr(name)?.extract() };
        if part("microsecond")? != 0 {
            return Err(PyValueError::new_err(format!(
                "time {time} has a fraction of a second; the envelope holds whole seconds"
            )));
        }
        let seconds = part("hour")? * 3600 + part("minute")? * 60 + part("second")?;
        let mut inner = Envelope::new(
            sender_qualifier,
            sender_id,
            receiver_qualifier,
            receiver_id,
            days,
            seconds,
        );
        inner.usage_indicator = usage_indicator.to_string();
        inner.control_number = control_number;
        inner.application_sender = application_sender;
        inner.application_receiver = application_receiver;
        inner.line_break = line_break;
        if let Some(delimiters) = delimiters {
            inner.delimiters = delimiters.get().inner;
        }
        Ok(Self { inner })
    }

    /// The sender id.
    #[getter]
    fn sender_id(&self) -> &str {
        &self.inner.sender_id
    }

    /// The sender id's qualifier.
    #[getter]
    fn sender_qualifier(&self) -> &str {
        &self.inner.sender_qualifier
    }

    /// The receiver id.
    #[getter]
    fn receiver_id(&self) -> &str {
        &self.inner.receiver_id
    }

    /// The receiver id's qualifier.
    #[getter]
    fn receiver_qualifier(&self) -> &str {
        &self.inner.receiver_qualifier
    }

    /// The usage indicator, e.g. `P` (production) or `T` (test).
    #[getter]
    fn usage_indicator(&self) -> &str {
        &self.inner.usage_indicator
    }

    /// The first control number.
    #[getter]
    fn control_number(&self) -> u64 {
        self.inner.control_number
    }

    /// The date.
    #[getter]
    #[gen_stub(override_return_type(type_repr = "datetime.date", imports = ("datetime",)))]
    fn date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        py.import("datetime")?
            .getattr("date")?
            .call_method1("fromordinal", (i64::from(self.inner.date) + EPOCH_ORDINAL,))
    }

    /// The time.
    #[getter]
    #[gen_stub(override_return_type(type_repr = "datetime.time", imports = ("datetime",)))]
    fn time<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let seconds = self.inner.time;
        py.import("datetime")?.getattr("time")?.call1((
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60,
        ))
    }

    /// The functional group's sender code, when it is not the sender id.
    #[getter]
    fn application_sender(&self) -> Option<&str> {
        self.inner.application_sender.as_deref()
    }

    /// The functional group's receiver code, when it is not the receiver id.
    #[getter]
    fn application_receiver(&self) -> Option<&str> {
        self.inner.application_receiver.as_deref()
    }

    /// Whether a line break follows each segment.
    #[getter]
    fn line_break(&self) -> bool {
        self.inner.line_break
    }

    /// The delimiters written.
    #[getter]
    fn delimiters(&self) -> PyDelimiters {
        PyDelimiters {
            inner: self.inner.delimiters,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Envelope(sender_id={:?}, receiver_id={:?}, control_number={})",
            self.inner.sender_id, self.inner.receiver_id, self.inner.control_number
        )
    }
}

/// One reason tables do not make a valid file. A value, never raised.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(name = "WriteFinding", module = "oxedi", frozen)]
pub struct PyWriteFinding {
    inner: Finding,
}

impl PyWriteFinding {
    /// Where the value at fault comes from.
    fn origin(&self) -> Option<&Origin> {
        match &self.inner {
            Finding::DelimiterInValue { origin, .. }
            | Finding::UnwrittenValue { origin, .. }
            | Finding::NotWritable { origin, .. } => Some(origin),
            Finding::ReadBack { origin, .. } => origin.as_ref(),
            _ => None,
        }
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PyWriteFinding {
    /// The kind of finding, e.g. `MissingParent` or `ReadBack`.
    #[getter]
    fn kind(&self) -> &'static str {
        match &self.inner {
            Finding::DelimiterInValue { .. } => "DelimiterInValue",
            Finding::UnwrittenValue { .. } => "UnwrittenValue",
            Finding::MissingParent { .. } => "MissingParent",
            Finding::MismatchedReference { .. } => "MismatchedReference",
            Finding::DuplicateRowNumber { .. } => "DuplicateRowNumber",
            Finding::OutOfOrder { .. } => "OutOfOrder",
            Finding::NotWritable { .. } => "NotWritable",
            Finding::ReadBack { .. } => "ReadBack",
        }
    }

    /// The table at fault, when a table is.
    #[getter]
    fn table(&self) -> Option<&str> {
        match &self.inner {
            Finding::MissingParent { table, .. }
            | Finding::MismatchedReference { table, .. }
            | Finding::DuplicateRowNumber { table, .. }
            | Finding::OutOfOrder { table, .. } => Some(table),
            _ => match self.origin()? {
                Origin::Cell { table, .. } | Origin::Row { table, .. } => Some(table),
                Origin::Envelope { .. } => None,
            },
        }
    }

    /// The row at fault (its position in the table), when a row is.
    #[getter]
    fn row(&self) -> Option<usize> {
        match &self.inner {
            Finding::MissingParent { row, .. }
            | Finding::MismatchedReference { row, .. }
            | Finding::DuplicateRowNumber { row, .. }
            | Finding::OutOfOrder { row, .. } => Some(*row),
            _ => match self.origin()? {
                Origin::Cell { row, .. } | Origin::Row { row, .. } => Some(*row),
                Origin::Envelope { .. } => None,
            },
        }
    }

    /// The column at fault, when a column is.
    #[getter]
    fn column(&self) -> Option<&str> {
        match &self.inner {
            Finding::MissingParent { column, .. }
            | Finding::MismatchedReference { column, .. }
            | Finding::OutOfOrder { column, .. } => Some(column),
            _ => match self.origin()? {
                Origin::Cell { column, .. } => Some(column),
                _ => None,
            },
        }
    }

    /// The envelope field at fault, when one is.
    #[getter]
    fn field(&self) -> Option<&str> {
        match self.origin()? {
            Origin::Envelope { field } => Some(field),
            _ => None,
        }
    }

    /// The diagnostic of reading the written file back, for a `ReadBack`.
    #[getter]
    fn diagnostic(&self) -> Option<PyDiagnostic> {
        match &self.inner {
            Finding::ReadBack { diagnostic, .. } => Some(PyDiagnostic::from(diagnostic.clone())),
            _ => None,
        }
    }

    /// The finding as one sentence: the rule, where and the value.
    #[getter]
    fn message(&self) -> String {
        self.inner.to_string()
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("WriteFinding({:?})", self.inner.to_string())
    }
}

/// Raises `oxedi.WriteError` with `message` and `findings`.
fn refuse(py: Python<'_>, message: String, findings: Vec<Finding>) -> PyErr {
    let raised = || -> PyResult<PyErr> {
        let list = findings_list(py, findings)?;
        let class = py.import("oxedi._write")?.getattr("WriteError")?;
        Ok(PyErr::from_value(class.call1((message, list))?))
    };
    raised().unwrap_or_else(|err| err)
}

fn findings_list(py: Python<'_>, findings: Vec<Finding>) -> PyResult<Bound<'_, PyList>> {
    let list = PyList::empty(py);
    for inner in findings {
        list.append(Py::new(py, PyWriteFinding { inner })?)?;
    }
    Ok(list)
}

/// Writes tables as one interchange; internal, see `oxedi.write`. Returns
/// the bytes and the findings, which are empty unless `allow_findings`.
#[gen_stub_pyfunction]
#[pyfunction]
#[pyo3(name = "_write", signature = (tables, envelope, spec = None, allow_findings = false))]
#[gen_stub(override_return_type(type_repr = "tuple[bytes, builtins.list[WriteFinding]]", imports = ("builtins",)))]
pub fn write_tables<'py>(
    py: Python<'py>,
    #[gen_stub(override_type(type_repr = "Tables | collections.abc.Mapping[builtins.str, typing.Any]", imports = ("builtins", "collections.abc", "typing")))]
    tables: &Bound<'py, PyAny>,
    envelope: &Bound<'py, PyEnvelope>,
    spec: Option<&Bound<'py, PySpec>>,
    #[gen_stub(override_type(type_repr = "builtins.bool", imports = ("builtins",)))]
    allow_findings: bool,
) -> PyResult<Bound<'py, PyTuple>> {
    let given = spec::given(spec);
    let (shared, spec): (Arc<Tables>, Arc<Spec>) = if let Ok(parsed) = tables.cast::<PyTables>() {
        let parsed = parsed.get();
        let spec = given
            .or_else(|| parsed.spec().cloned())
            .unwrap_or_else(spec::builtin);
        (Arc::clone(parsed.tables()), spec)
    } else {
        let mapping = tables.cast::<PyMapping>().map_err(|_| {
                refuse(
                    py,
                    "write: tables must be the tables of a parse or a mapping of table names to Arrow, Polars or pandas tables".to_string(),
                    Vec::new(),
                )
            })?;
        let spec = given.unwrap_or_else(spec::builtin);
        match import::tables(&spec, mapping)? {
            Ok(tables) => (Arc::new(tables), spec),
            Err(message) => return Err(refuse(py, message, Vec::new())),
        }
    };
    let envelope = envelope.get().inner.clone();
    let written = py.detach(|| write_with_findings(&spec, &shared, &envelope));
    let (bytes, findings) = match written {
        Ok(written) => written,
        Err(error) => {
            let findings = match &error {
                WriteError::Findings(findings) => findings.clone(),
                _ => Vec::new(),
            };
            return Err(refuse(py, error.to_string(), findings));
        }
    };
    if !allow_findings && !findings.is_empty() {
        let message = WriteError::Findings(findings.clone()).to_string();
        return Err(refuse(py, message, findings));
    }
    let list = findings_list(py, findings)?;
    PyTuple::new(py, [PyBytes::new(py, &bytes).into_any(), list.into_any()])
}

pyo3_stub_gen::export_verbatim!("oxedi._core", "_write");
