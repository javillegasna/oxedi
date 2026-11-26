//! `parse`: one pass over a whole file, with the GIL released.

use oxedi_core::trivia::LeadingTrivia;
use oxedi_core::{Delimiters, Document, Mask, Processor, engine};

use pyo3::buffer::PyBuffer;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyString};
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyfunction, gen_stub_pymethods};

use crate::diagnostic;
use crate::document::{self, PyDelimiters, PyDocument};
use crate::mask::PyMask;
use crate::native;
use crate::spec::{self, HeldSpec, PySpec};
use crate::tables::PyTables;

/// Copies the input exactly once into memory Rust owns. Any object with the
/// buffer protocol is accepted (`bytes`, `bytearray`, `memoryview`, `mmap`).
pub fn copy_input(function: &str, data: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    if data.is_instance_of::<PyString>() {
        return Err(PyTypeError::new_err(
            "the input must be bytes or another buffer, not str: open the file in binary mode or encode the text",
        ));
    }
    match PyBuffer::<u8>::get(data) {
        Ok(buffer) => buffer.to_vec(data.py()),
        Err(_) => Err(PyTypeError::new_err(format!(
            "{function}: argument data must be a buffer of unsigned bytes, format 'B', 'b' or 'c'; found {}",
            found(data)
        ))),
    }
}

/// What the object is, for the refusal: its buffer format when it has one,
/// else its type name.
fn found(data: &Bound<'_, PyAny>) -> String {
    let format = data
        .py()
        .import("builtins")
        .and_then(|builtins| builtins.getattr("memoryview"))
        .and_then(|view| view.call1((data,)))
        .and_then(|view| view.getattr("format"))
        .and_then(|format| format.repr())
        .map(|format| format.to_string());
    match format {
        Ok(format) => format!("format {format}"),
        Err(_) => format!(
            "type {}",
            data.get_type()
                .name()
                .map_or_else(|_| "?".to_string(), |name| name.to_string())
        ),
    }
}

/// What one parse produced.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(name = "Result", module = "oxedi", frozen)]
pub struct PyParseResult {
    document: Py<PyDocument>,
    tables: Py<PyTables>,
    diagnostics: Py<PyList>,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyParseResult {
    /// The file, held losslessly.
    #[getter]
    fn document(&self, py: Python<'_>) -> Py<PyDocument> {
        self.document.clone_ref(py)
    }

    /// The projected tables, by name.
    #[getter]
    fn tables(&self, py: Python<'_>) -> Py<PyTables> {
        self.tables.clone_ref(py)
    }

    /// The spec that projected the tables: the one given, or the built-in
    /// selected from the file's version.
    #[getter]
    fn spec(&self, py: Python<'_>) -> PySpec {
        PySpec::from(self.tables.bind(py).get().spec().clone())
    }

    /// Every diagnostic, in stream order.
    #[getter]
    #[gen_stub(override_return_type(type_repr = "builtins.list[Diagnostic]", imports = ("builtins",)))]
    fn diagnostics(&self, py: Python<'_>) -> Py<PyList> {
        self.diagnostics.clone_ref(py)
    }

    /// The summary `name` the spec declares, as the value of its kind: an
    /// `int` for a `count` or `distinct` summary (a `distinct` one compares
    /// values as text, so `0123` and `123` are two), a `Decimal` at the
    /// column's scale for a `sum`, a dict of a row's fields for a `row`, or
    /// `None` for a `row` the file holds nothing of. Raises `KeyError` for a
    /// summary the spec does not declare or a table or column it lacks,
    /// `TypeError` for a `sum` over a column that is not decimal, and
    /// `ValueError` for a `row` over a table without exactly one row or a sum
    /// that overflows.
    #[gen_stub(override_return_type(type_repr = "builtins.int | decimal.Decimal | builtins.dict[builtins.str, builtins.str | None] | None", imports = ("builtins", "decimal")))]
    fn summary<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        native::value(py, self.tables.bind(py).get(), name)
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        format!(
            "Result(segments={}, tables={}, diagnostics={})",
            self.document.bind(py).get().inner.len(),
            self.tables.bind(py).get().count(),
            self.diagnostics.bind(py).len()
        )
    }
}

/// Parses a whole file: indexes every segment, runs the loop engine, the
/// structure checker and the projector, and returns the document, the
/// tables and every diagnostic. Without `spec`, the built-in spec of the
/// version the file declares is used, else the default one. With `mask`, the
/// patient data of the tables and diagnostics is hidden.
#[gen_stub_pyfunction]
#[pyfunction]
#[pyo3(signature = (data, spec = None, delimiters = None, mask = None))]
pub fn parse(
    py: Python<'_>,
    #[gen_stub(override_type(type_repr = "typing_extensions.Buffer", imports = ("typing_extensions",)))]
    data: &Bound<'_, PyAny>,
    spec: Option<&Bound<'_, PySpec>>,
    delimiters: Option<&Bound<'_, PyDelimiters>>,
    mask: Option<&Bound<'_, PyMask>>,
) -> PyResult<PyParseResult> {
    let bytes = copy_input("parse", data)?;
    let pass = Pass::new(spec, delimiters, mask);
    let (document, tables, diagnostics) = py.detach(|| pass.run(bytes))?;
    PyParseResult::new(py, document, tables, diagnostics)
}

/// What a parse is configured with, as core values: the Python wrappers are
/// opened once, on the thread that holds the GIL, so the pass itself needs
/// none.
struct Pass {
    given: Option<HeldSpec>,
    delimiters: Option<Delimiters>,
    mask: Option<Mask>,
}

impl Pass {
    fn new(
        spec: Option<&Bound<'_, PySpec>>,
        delimiters: Option<&Bound<'_, PyDelimiters>>,
        mask: Option<&Bound<'_, PyMask>>,
    ) -> Pass {
        Pass {
            given: spec::given(spec),
            delimiters: delimiters.map(|d| d.get().inner),
            mask: mask.map(|mask| mask.get().inner.clone()),
        }
    }

    /// Indexes `bytes`, then runs the processor with the spec given or the one
    /// the file's version selects.
    fn run(
        self,
        bytes: Vec<u8>,
    ) -> PyResult<(Document<'static>, PyTables, Vec<oxedi_core::Diagnostic>)> {
        let document = document::index(bytes, self.delimiters)?;
        let spec = spec::given_or_selected(self.given, document.segments());
        let (tables, diagnostics) = Processor::run(&spec, &document, self.mask.as_ref());
        Ok((document, PyTables::projected(tables, spec), diagnostics))
    }
}

impl PyParseResult {
    /// Wraps the parts of one parse as Python objects.
    fn new(
        py: Python<'_>,
        document: Document<'static>,
        tables: PyTables,
        diagnostics: Vec<oxedi_core::Diagnostic>,
    ) -> PyResult<PyParseResult> {
        Ok(PyParseResult {
            document: Py::new(py, PyDocument { inner: document })?,
            tables: Py::new(py, tables)?,
            diagnostics: diagnostic::to_list(py, diagnostics)?.unbind(),
        })
    }
}

/// Indexes a whole file without running the loop engine or the projector:
/// the document, the spec a `parse` of the same arguments uses (the one
/// given, or the built-in of the version the file declares) and how many
/// bytes precede the first segment's text (the byte order mark and the
/// whitespace before the ISA).
///
/// Internal: adapters to external validators read a file's document and
/// spec through it without paying for the tables.
#[gen_stub_pyfunction]
#[pyfunction]
#[pyo3(name = "_parse_document", signature = (data, spec = None))]
pub fn parse_document(
    py: Python<'_>,
    #[gen_stub(override_type(type_repr = "typing_extensions.Buffer", imports = ("typing_extensions",)))]
    data: &Bound<'_, PyAny>,
    spec: Option<&Bound<'_, PySpec>>,
) -> PyResult<(PyDocument, PySpec, usize)> {
    let bytes = copy_input("_parse_document", data)?;
    let given = spec::given(spec);
    let (document, spec, leading) = py.detach(|| -> PyResult<_> {
        let leading = LeadingTrivia::of(&bytes).len();
        let document = document::index(bytes, None)?;
        let spec = spec::given_or_selected(given, document.segments());
        Ok((document, spec, leading))
    })?;
    Ok((PyDocument { inner: document }, PySpec::from(spec), leading))
}

/// The open loops at each of `segments`, outermost first, as `(name,
/// ordinal)` pairs: the loops `parse` names in the `path` of a diagnostic
/// about that segment. The ordinal of an instance counts every instance of
/// its loop from the start of the stream.
///
/// Internal: adapters to external validators call it to place their
/// findings; it is not part of the public API. An index past the last
/// segment is a `ValueError` naming it.
#[gen_stub_pyfunction]
#[pyfunction]
#[pyo3(name = "_loop_paths")]
pub fn loop_paths(
    document: &Bound<'_, PyDocument>,
    spec: &Bound<'_, PySpec>,
    segments: Vec<usize>,
) -> PyResult<Vec<Vec<(String, usize)>>> {
    if segments.is_empty() {
        return Ok(Vec::new());
    }
    let document = &document.get().inner;
    let count = document.len();
    if let Some(&past) = segments.iter().find(|&&index| index >= count) {
        return Err(PyValueError::new_err(format!(
            "_loop_paths: segment {past} is past the document's last segment ({count} segments)"
        )));
    }
    let spec = &spec.get().inner;
    let paths = engine::loop_paths(spec, document.segments(), &segments);
    Ok(paths
        .into_iter()
        .map(|path| {
            path.into_iter()
                .map(|open| {
                    (
                        spec.loop_name(open.loop_id).to_string(),
                        open.ordinal_in_stream,
                    )
                })
                .collect()
        })
        .collect())
}

pyo3_stub_gen::export_verbatim!("oxedi._core", "_loop_paths");
pyo3_stub_gen::export_verbatim!("oxedi._core", "_parse_document");
