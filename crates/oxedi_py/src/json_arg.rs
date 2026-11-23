//! The JSON argument of `Spec.patch`: JSON text, or a Python object checked to
//! be a JSON object before it is written as text.

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyMapping, PyString, PyTuple};

/// How deep a patch may nest. A cycle never ends, so the walk stops here and
/// names the place instead of overflowing the stack.
const MAX_DEPTH: usize = 128;

/// A patch argument: JSON text, or the reason it is refused.
pub enum PatchArg {
    Json(String),
    Refused(String),
}

/// One step into a nested value: a mapping key or a list position.
enum Step {
    Key(String),
    Index(usize),
}

/// Why a walk stopped: Python raised, or the value is not JSON.
enum Failure {
    Python(PyErr),
    Refused(String),
}

impl From<PyErr> for Failure {
    fn from(err: PyErr) -> Failure {
        Failure::Python(err)
    }
}

type Walk<T> = Result<T, Failure>;

impl PatchArg {
    /// The JSON text of the patch, or the `TypeError` `Spec.patch` raises.
    pub fn into_json(self) -> PyResult<String> {
        match self {
            PatchArg::Json(text) => Ok(text),
            PatchArg::Refused(message) => Err(PyTypeError::new_err(message)),
        }
    }

    /// A checked mapping written as JSON text by Python's `json.dumps`, which
    /// never converts a `NaN` or an infinity because the walk refused them.
    fn dumped(value: &Bound<'_, PyAny>) -> PyResult<PatchArg> {
        let py = value.py();
        let kwargs = PyDict::new(py);
        kwargs.set_item("allow_nan", false)?;
        let text = py
            .import("json")?
            .call_method("dumps", (value,), Some(&kwargs))?
            .extract::<String>()?;
        Ok(PatchArg::Json(text))
    }

    /// A mapping, checked and written as JSON text.
    fn from_mapping(mapping: &Bound<'_, PyMapping>) -> PyResult<PatchArg> {
        match check(mapping.as_any(), &mut Vec::new()) {
            Ok(plain) => PatchArg::dumped(&plain),
            Err(Failure::Refused(message)) => Ok(PatchArg::Refused(message)),
            Err(Failure::Python(err)) => Err(err),
        }
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for PatchArg {
    type Error = PyErr;

    fn extract(object: Borrowed<'a, 'py, PyAny>) -> PyResult<PatchArg> {
        if let Ok(text) = object.cast::<PyString>() {
            return Ok(PatchArg::Json(text.to_cow()?.into_owned()));
        }
        if let Ok(mapping) = object.cast::<PyMapping>() {
            return PatchArg::from_mapping(&mapping);
        }
        Ok(PatchArg::Refused(format!(
            "Spec.patch takes a dict or a JSON string, not {}",
            kind(&object)?
        )))
    }
}

/// The type name of `value`, as Python prints it.
fn kind(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(value.get_type().name()?.to_string())
}

/// `path` as the text an error names: keys joined with `.`, a list position
/// as `[i]` with no dot; the top of the patch when empty.
fn place(path: &[Step]) -> String {
    let mut text = String::new();
    for step in path {
        match step {
            Step::Key(key) if text.is_empty() => text.push_str(key),
            Step::Key(key) => {
                text.push('.');
                text.push_str(key);
            }
            Step::Index(index) => text.push_str(&format!("[{index}]")),
        }
    }
    if text.is_empty() {
        "the top of the patch".to_string()
    } else {
        text
    }
}

fn refuse<T>(message: String) -> Walk<T> {
    Err(Failure::Refused(format!("Spec.patch: {message}")))
}

/// A copy of `value` made of plain dicts, lists and scalars when every part is
/// a JSON value, or the first part that is not. The copy is what Python's
/// `json` can write, whatever mapping or tuple the caller used.
fn check<'py>(value: &Bound<'py, PyAny>, path: &mut Vec<Step>) -> Walk<Bound<'py, PyAny>> {
    if path.len() > MAX_DEPTH {
        return refuse(format!(
            "the value at {} nests deeper than {MAX_DEPTH} levels (a cycle, or deeper than any spec)",
            place(&path[..MAX_DEPTH])
        ));
    }
    if let Some(scalar) = check_scalar(value, path)? {
        return Ok(scalar);
    }
    check_container(value, path)
}

/// `check` for a mapping, a list or a tuple; anything else is not JSON.
fn check_container<'py>(
    value: &Bound<'py, PyAny>,
    path: &mut Vec<Step>,
) -> Walk<Bound<'py, PyAny>> {
    if let Ok(mapping) = value.cast::<PyMapping>() {
        return check_mapping(mapping, path);
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        return check_sequence(value, path);
    }
    refuse(format!(
        "the value at {} has type {}, which is not a JSON value",
        place(path),
        kind(value)?
    ))
}

/// The value itself when it is `None`, a `bool`, a `str`, an integer within 64
/// bits or a finite float; `None` when it is not a scalar; a refusal when it is
/// a number JSON cannot hold.
fn check_scalar<'py>(value: &Bound<'py, PyAny>, path: &[Step]) -> Walk<Option<Bound<'py, PyAny>>> {
    let plain =
        value.is_none() || value.is_instance_of::<PyBool>() || value.is_instance_of::<PyString>();
    if plain {
        return Ok(Some(value.clone()));
    }
    if value.is_instance_of::<PyInt>() {
        let fits = value.extract::<i64>().is_ok() || value.extract::<u64>().is_ok();
        return number(
            value,
            path,
            fits,
            "outside the 64-bit integers a spec reads",
        );
    }
    if let Ok(float) = value.cast::<PyFloat>() {
        let finite = float.value().is_finite();
        return number(value, path, finite, "which JSON cannot hold");
    }
    Ok(None)
}

fn number<'py>(
    value: &Bound<'py, PyAny>,
    path: &[Step],
    accepted: bool,
    reason: &str,
) -> Walk<Option<Bound<'py, PyAny>>> {
    if accepted {
        return Ok(Some(value.clone()));
    }
    refuse(format!(
        "the value at {} is {}, {reason}",
        place(path),
        value.repr()?
    ))
}

/// A dict of the mapping's entries, every key a `str`.
fn check_mapping<'py>(
    mapping: &Bound<'py, PyMapping>,
    path: &mut Vec<Step>,
) -> Walk<Bound<'py, PyAny>> {
    let plain = PyDict::new(mapping.py());
    for item in mapping.items()?.try_iter()? {
        let (name, checked) = check_entry(&item?, path)?;
        plain.set_item(name, checked)?;
    }
    Ok(plain.into_any())
}

/// One `(key, value)` pair of a mapping: the key, which must be a `str`, and
/// the checked value.
fn check_entry<'py>(
    item: &Bound<'py, PyAny>,
    path: &mut Vec<Step>,
) -> Walk<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let (key, value) = item.extract::<(Bound<'py, PyAny>, Bound<'py, PyAny>)>()?;
    let Ok(name) = key.cast::<PyString>() else {
        return refuse(format!(
            "a key at {} has type {}; keys must be str",
            place(path),
            kind(&key)?
        ));
    };
    path.push(Step::Key(name.to_cow()?.into_owned()));
    let checked = check(&value, path)?;
    path.pop();
    Ok((key, checked))
}

/// A list of a list's or tuple's items.
fn check_sequence<'py>(
    sequence: &Bound<'py, PyAny>,
    path: &mut Vec<Step>,
) -> Walk<Bound<'py, PyAny>> {
    let plain = PyList::empty(sequence.py());
    for (index, item) in sequence.try_iter()?.enumerate() {
        path.push(Step::Index(index));
        let checked = check(&item?, path)?;
        path.pop();
        plain.append(checked)?;
    }
    Ok(plain.into_any())
}
