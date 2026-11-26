//! `EnvelopeLoop`: a loop of the spec's envelope, as a Python value.

use oxedi_core::Spec;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyString};
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};

/// A loop of the envelope: its end segment checks the loop it closes by a
/// control number and a count.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(
    name = "EnvelopeLoop",
    module = "oxedi",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyEnvelopeLoop {
    name: String,
    trigger: Vec<u8>,
    end: Vec<u8>,
    opener_element: usize,
    closer_element: usize,
    count_element: usize,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyEnvelopeLoop {
    /// The loop's name in the spec, such as `"interchange"`.
    #[getter]
    fn name(&self) -> &str {
        &self.name
    }

    /// The id of the segment that opens the loop, such as `b"ISA"`.
    #[getter]
    fn trigger<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.trigger)
    }

    /// The id of the segment that closes the loop, such as `b"IEA"`.
    #[getter]
    fn end<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.end)
    }

    /// The 1-based position of the control number in the trigger segment,
    /// such as 13 for ISA13.
    #[getter]
    fn opener_element(&self) -> usize {
        self.opener_element
    }

    /// The 1-based position of the same control number in the end segment,
    /// such as 2 for IEA02.
    #[getter]
    fn closer_element(&self) -> usize {
        self.closer_element
    }

    /// The 1-based position of the count in the end segment, such as 1 for
    /// IEA01.
    #[getter]
    fn count_element(&self) -> usize {
        self.count_element
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let name = PyString::new(py, &self.name).repr()?;
        let trigger = PyBytes::new(py, &self.trigger).repr()?;
        let end = PyBytes::new(py, &self.end).repr()?;
        Ok(format!(
            "EnvelopeLoop(name={name}, trigger={trigger}, end={end}, opener_element={}, closer_element={}, count_element={})",
            self.opener_element, self.closer_element, self.count_element
        ))
    }
}

/// The envelope loops of `spec`, outermost first: a pre-order walk from the
/// roots, in spec order among siblings, keeping each loop that has both a
/// control and an end segment.
pub fn envelope(spec: &Spec) -> Vec<PyEnvelopeLoop> {
    let mut pending: Vec<_> = spec.roots().iter().rev().copied().collect();
    let mut found = Vec::new();
    while let Some(id) = pending.pop() {
        let def = spec.get(id);
        if let (Some(control), Some(end)) = (def.control, def.end.as_ref()) {
            found.push(PyEnvelopeLoop {
                name: def.name.clone(),
                trigger: def.trigger.segment.clone(),
                end: end.clone(),
                opener_element: control.opener_element,
                closer_element: control.closer_element,
                count_element: control.count_element,
            });
        }
        pending.extend(def.children.iter().rev().copied());
    }
    found
}
