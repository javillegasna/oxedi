//! `Document`, `Segment` and `Delimiters`: the file held losslessly.

use edi835_core::{Delimiters, Document, Element};
use pyo3::exceptions::{PyIndexError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyList};
use pyo3_stub_gen::derive::{gen_methods_from_python, gen_stub_pyclass, gen_stub_pymethods};
use pyo3_stub_gen::type_info::{
    MemberInfo, MethodInfo, MethodType, ParameterDefault, ParameterInfo, ParameterKind,
    PyMethodsInfo,
};
use pyo3_stub_gen::{PyStubType, TypeInfo};

native_exception!(
    ParseError,
    PyValueError,
    "Input that cannot be indexed: its delimiters cannot be read or it exceeds the size limit; the message says what was found and where."
);

/// The five delimiters of an interchange.
#[gen_stub_pyclass(module = "oxedi835._core")]
#[pyclass(
    name = "Delimiters",
    module = "oxedi835",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyDelimiters {
    pub inner: Delimiters,
}

/// A delimiter parameter of `Delimiters.__new__`: `bytes`, defaulting to
/// `default` written as a Python literal.
const fn delimiter(name: &'static str, default: fn() -> String) -> ParameterInfo {
    ParameterInfo {
        name,
        kind: ParameterKind::PositionalOrKeyword,
        type_info: <Py<PyBytes> as PyStubType>::type_input,
        default: ParameterDefault::Expr {
            value: default,
            source_module: None,
        },
    }
}

/// An optional delimiter parameter of `Delimiters.__new__`: `bytes` or
/// `None`, defaulting to `None`.
const fn optional_delimiter(name: &'static str) -> ParameterInfo {
    ParameterInfo {
        name,
        kind: ParameterKind::PositionalOrKeyword,
        type_info: <Option<Py<PyBytes>> as PyStubType>::type_input,
        default: ParameterDefault::Expr {
            value: || "None".to_string(),
            source_module: None,
        },
    }
}

// `Delimiters.__new__` is described by hand: its byte-string defaults have
// no Python form the derive macros can write. It mirrors `signature =` on
// `PyDelimiters::new`. `__hash__` is `None`: the class compares by value and
// is not hashable.
pyo3_stub_gen::inventory::submit! {
    PyMethodsInfo {
        struct_id: std::any::TypeId::of::<PyDelimiters>,
        attrs: &[MemberInfo {
            name: "__hash__",
            // Overriding `object.__hash__` with `None` is how typeshed spells an
            // unhashable class; the ignore keeps type checkers from flagging it.
            r#type: || {
                TypeInfo::with_module(
                    "typing.ClassVar[None]  # type: ignore[assignment]",
                    "typing".into(),
                )
            },
            doc: "",
            default: None,
            deprecated: None,
        }],
        getters: &[],
        setters: &[],
        methods: &[MethodInfo {
            name: "__new__",
            parameters: &[
                delimiter("element", || "b\"*\"".to_string()),
                delimiter("component", || "b\":\"".to_string()),
                delimiter("segment", || "b\"~\"".to_string()),
                optional_delimiter("repetition"),
                optional_delimiter("release"),
            ],
            r#return: || TypeInfo::unqualified("Delimiters"),
            doc: "",
            r#type: MethodType::New,
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

/// The bytes as Python writes them, e.g. `b'*'` or `b'\n'`.
fn bytes_repr(py: Python<'_>, bytes: &[u8]) -> PyResult<String> {
    Ok(PyBytes::new(py, bytes).repr()?.to_string())
}

fn one_byte(py: Python<'_>, name: &str, value: &[u8]) -> PyResult<u8> {
    match value {
        [byte] => Ok(*byte),
        _ => Err(PyValueError::new_err(format!(
            "delimiter {name} must be exactly one byte, got {} bytes: {}",
            value.len(),
            bytes_repr(py, value)?
        ))),
    }
}

fn optional_repr(py: Python<'_>, byte: Option<u8>) -> PyResult<String> {
    byte.map_or_else(|| Ok("None".to_string()), |b| bytes_repr(py, &[b]))
}

fn single<'py>(py: Python<'py>, byte: u8) -> Bound<'py, PyBytes> {
    PyBytes::new(py, &[byte])
}

#[gen_stub_pymethods]
#[pymethods]
impl PyDelimiters {
    #[new]
    #[gen_stub(skip)]
    // `text_signature` mirrors `signature =`, as the stub description above does.
    #[pyo3(
        signature = (element = b"*".as_slice(), component = b":".as_slice(), segment = b"~".as_slice(), repetition = None, release = None),
        text_signature = "(element=b'*', component=b':', segment=b'~', repetition=None, release=None)"
    )]
    fn new(
        py: Python<'_>,
        element: &[u8],
        component: &[u8],
        segment: &[u8],
        repetition: Option<&[u8]>,
        release: Option<&[u8]>,
    ) -> PyResult<Self> {
        let mut inner = Delimiters::new(
            one_byte(py, "element", element)?,
            one_byte(py, "component", component)?,
            one_byte(py, "segment", segment)?,
        );
        inner.repetition = repetition
            .map(|value| one_byte(py, "repetition", value))
            .transpose()?;
        inner.release = release
            .map(|value| one_byte(py, "release", value))
            .transpose()?;
        Ok(Self { inner })
    }

    /// The element separator, one byte.
    #[getter]
    fn element<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        single(py, self.inner.element)
    }

    /// The component separator, one byte.
    #[getter]
    fn component<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        single(py, self.inner.component)
    }

    /// The segment terminator, one byte.
    #[getter]
    fn segment<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        single(py, self.inner.segment)
    }

    /// The repetition separator, or `None` when the file has none.
    #[getter]
    fn repetition<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyBytes>> {
        self.inner.repetition.map(|byte| single(py, byte))
    }

    /// The release character, or `None` when it is not in use.
    #[getter]
    fn release<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyBytes>> {
        self.inner.release.map(|byte| single(py, byte))
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "Delimiters(element={}, component={}, segment={}, repetition={}, release={})",
            bytes_repr(py, &[self.inner.element])?,
            bytes_repr(py, &[self.inner.component])?,
            bytes_repr(py, &[self.inner.segment])?,
            optional_repr(py, self.inner.repetition)?,
            optional_repr(py, self.inner.release)?
        ))
    }
}

/// The file, indexed into segments and held byte for byte.
#[gen_stub_pyclass(module = "oxedi835._core")]
#[pyclass(name = "Document", module = "oxedi835", frozen)]
pub struct PyDocument {
    pub inner: Document<'static>,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyDocument {
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    #[gen_stub(skip)]
    fn __getitem__(slf: &Bound<'_, Self>, index: isize) -> PyResult<PySegment> {
        let len = slf.get().inner.len();
        let resolved = if index < 0 {
            len.checked_sub(index.unsigned_abs())
        } else {
            Some(index.unsigned_abs()).filter(|position| *position < len)
        };
        match resolved {
            Some(index) => Ok(PySegment {
                document: slf.clone().unbind(),
                index,
            }),
            None => Err(PyIndexError::new_err(format!(
                "segment index {index} is out of range: the document has {len} segments"
            ))),
        }
    }

    /// The file, byte for byte.
    fn write<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, self.inner.as_bytes())
    }

    /// The delimiters the file was indexed with.
    #[getter]
    fn delimiters(&self) -> PyDelimiters {
        PyDelimiters {
            inner: *self.inner.delimiters(),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Document(segments={}, bytes={})",
            self.inner.len(),
            self.inner.as_bytes().len()
        )
    }
}

// Slot methods take their argument by position only.
pyo3_stub_gen::inventory::submit! {
    gen_methods_from_python! {
        r#"
        class PyDocument:
            def __getitem__(self, index: builtins.int, /) -> Segment: ...
        "#
    }
}

/// One segment of a document, read from the document on access.
#[gen_stub_pyclass(module = "oxedi835._core")]
#[pyclass(name = "Segment", module = "oxedi835", frozen)]
pub struct PySegment {
    document: Py<PyDocument>,
    index: usize,
}

impl PySegment {
    fn with<T>(
        &self,
        py: Python<'_>,
        read: impl FnOnce(&edi835_core::Segment<'_>) -> T,
    ) -> PyResult<T> {
        let document = self.document.bind(py).get();
        document
            .inner
            .segment(self.index)
            .map(|segment| read(&segment))
            .ok_or_else(|| out_of_range(self.index, document.inner.len()))
    }
}

fn out_of_range(index: usize, len: usize) -> PyErr {
    PyIndexError::new_err(format!(
        "segment index {index} is out of range: the document has {len} segments"
    ))
}

#[gen_stub_pymethods]
#[pymethods]
impl PySegment {
    /// The 0-based position of the segment in the document.
    #[getter]
    fn index(&self) -> usize {
        self.index
    }

    /// The segment identifier; empty for an empty frame.
    #[getter]
    fn id<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        self.with(py, |segment| PyBytes::new(py, segment.id))
    }

    /// The elements in X12 order (`elements[0]` is `XX01`): `bytes` for a
    /// simple element, a list of `bytes` for a composite one.
    #[getter]
    #[gen_stub(override_return_type(type_repr = "builtins.list[builtins.bytes | builtins.list[builtins.bytes]]", imports = ("builtins",)))]
    fn elements<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        self.with(py, |segment| -> PyResult<Bound<'py, PyList>> {
            let list = PyList::empty(py);
            for element in &segment.elements {
                match element {
                    Element::Simple(value) => list.append(PyBytes::new(py, value))?,
                    Element::Composite(values) => {
                        let parts = PyList::empty(py);
                        for value in values {
                            parts.append(PyBytes::new(py, value))?;
                        }
                        list.append(parts)?;
                    }
                }
            }
            Ok(list)
        })?
    }

    /// The exact bytes of the segment: leading trivia, body and terminator.
    #[getter]
    fn raw<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        self.with(py, |segment| PyBytes::new(py, segment.raw))
    }

    /// The segment's byte range in the input, `(start, end)` with `end`
    /// exclusive: leading trivia, body and terminator, as `raw` holds them.
    #[getter]
    fn span(&self, py: Python<'_>) -> PyResult<(usize, usize)> {
        let document = self.document.bind(py).get();
        document
            .inner
            .span(self.index)
            .map(|span| (span.raw.start, span.raw.end))
            .ok_or_else(|| out_of_range(self.index, document.inner.len()))
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        self.with(py, |segment| -> PyResult<String> {
            Ok(format!(
                "Segment(index={}, id={})",
                segment.index,
                bytes_repr(py, segment.id)?
            ))
        })?
    }
}

/// Indexes owned bytes, reading the delimiters from the ISA or using the
/// ones given.
pub fn index(bytes: Vec<u8>, delimiters: Option<Delimiters>) -> PyResult<Document<'static>> {
    match delimiters {
        Some(delimiters) => Document::with_delimiters(bytes, delimiters)
            .map_err(|err| ParseError::new_err(err.to_string())),
        None => Document::parse(bytes).map_err(|err| ParseError::new_err(err.to_string())),
    }
}
