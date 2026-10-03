//! `stream`: the tables of a file in batches, one per closed loop instance.
//!
//! The stream owns a copy of the input and walks it with the tokenizer, so
//! no document index is built; after each instance of the chosen loop closes
//! it moves the rows appended so far out of the projector. What it holds at
//! any time is the input, the open loops and the rows of one batch.

use std::sync::Arc;

use edi835_core::{Delimiters, Diagnostic, Event, LoopId, Processor, Spec, Tables, Tokenizer};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyList;
use self_cell::self_cell;

use crate::diagnostic;
use crate::document::{ParseError, PyDelimiters};
use crate::parse::copy_input;
use crate::spec::{self, PySpec};
use crate::tables::PyTables;

/// What the walk borrows from: the spec, the input and its delimiters.
struct Source {
    spec: Arc<Spec>,
    bytes: Vec<u8>,
    delimiters: Delimiters,
}

/// The tokenizer over the input and the processor over the spec.
struct Pass<'a> {
    segments: Tokenizer<'a>,
    processor: Processor<'a>,
}

self_cell!(
    struct Walk {
        owner: Source,
        #[covariant]
        dependent: Pass,
    }
);

/// One batch: the tables and diagnostics produced since the previous one.
type Produced = (Tables, Vec<Diagnostic>);

struct State {
    walk: Walk,
    by: LoopId,
    done: bool,
}

impl State {
    /// Feeds segments until an instance of `by` closes, or finishes the
    /// stream at its end. `None` once the stream is over and nothing is left.
    fn step(&mut self) -> Option<Produced> {
        if self.done {
            return None;
        }
        let by = self.by;
        let (produced, done) = self.walk.with_dependent_mut(|_, pass| {
            let mut diagnostics = Vec::new();
            for segment in pass.segments.by_ref() {
                let output = pass.processor.feed(&segment);
                diagnostics.extend_from_slice(output.diagnostics());
                if output.events().contains(&Event::LoopClosed { id: by }) {
                    return ((pass.processor.take_tables(), diagnostics), false);
                }
            }
            diagnostics.extend_from_slice(pass.processor.finish().diagnostics());
            ((pass.processor.take_tables(), diagnostics), true)
        });
        self.done = done;
        let empty = produced.0.iter().all(|table| table.is_empty()) && produced.1.is_empty();
        if done && empty { None } else { Some(produced) }
    }
}

/// Iterates the batches of a file. Each step runs with the GIL released.
#[pyclass(name = "Stream", module = "oxedi835")]
pub struct PyStream {
    state: State,
}

/// The tables and diagnostics of one closed loop instance (or of the end
/// of the stream).
#[pyclass(name = "Batch", module = "oxedi835", frozen)]
pub struct PyBatch {
    tables: Py<PyTables>,
    diagnostics: Py<PyList>,
}

#[pymethods]
impl PyBatch {
    /// The rows appended since the previous batch, by table.
    #[getter]
    fn tables(&self, py: Python<'_>) -> Py<PyTables> {
        self.tables.clone_ref(py)
    }

    /// The diagnostics emitted since the previous batch, in stream order.
    #[getter]
    fn diagnostics(&self, py: Python<'_>) -> Py<PyList> {
        self.diagnostics.clone_ref(py)
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "Batch(tables={}, diagnostics={})",
            self.tables.bind(py).repr()?,
            self.diagnostics.bind(py).len()
        ))
    }
}

#[pymethods]
impl PyStream {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<PyBatch>> {
        let state = &mut self.state;
        let Some((tables, diagnostics)) = py.detach(|| state.step()) else {
            return Ok(None);
        };
        let diagnostics = diagnostic::to_list(py, diagnostics)?;
        Ok(Some(PyBatch {
            tables: Py::new(py, PyTables::from(tables))?,
            diagnostics: diagnostics.unbind(),
        }))
    }
}

/// Walks a file and yields a batch each time an instance of loop `by`
/// closes, and one more at the end when anything is left.
#[pyfunction]
#[pyo3(signature = (data, spec = None, by = "transaction", delimiters = None))]
pub fn stream(
    py: Python<'_>,
    data: &Bound<'_, PyAny>,
    spec: Option<&Bound<'_, PySpec>>,
    by: &str,
    delimiters: Option<&Bound<'_, PyDelimiters>>,
) -> PyResult<PyStream> {
    let bytes = copy_input(data)?;
    let spec = spec::or_builtin(spec);
    let Some(by) = spec.loop_id(by) else {
        let names: Vec<&str> = spec.loops().iter().map(|l| l.name.as_str()).collect();
        return Err(PyValueError::new_err(format!(
            "stream by {by:?}: the spec has no such loop; its loops are {}",
            names.join(", ")
        )));
    };
    let delimiters = match delimiters {
        Some(delimiters) => delimiters.get().inner,
        None => *Tokenizer::new(&bytes)
            .map_err(|err| ParseError::new_err(err.to_string()))?
            .delimiters(),
    };
    let source = Source {
        spec,
        bytes,
        delimiters,
    };
    let walk = py.detach(|| {
        Walk::new(source, |source| Pass {
            segments: Tokenizer::with_delimiters(&source.bytes, source.delimiters),
            processor: Processor::new(&source.spec, &source.delimiters),
        })
    });
    Ok(PyStream {
        state: State {
            walk,
            by,
            done: false,
        },
    })
}
