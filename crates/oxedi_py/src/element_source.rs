//! `ElementSource` and `GroupSource`: where table columns read their values, as Python attributes.

use std::collections::BTreeMap;

use oxedi_core::{ColumnSource, Repeat, Spec, TableDef};
use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};

/// Where a table column reads its value: one element, or one component of it,
/// of a segment. It names only the segment, element and component a column
/// reads, not the qualifier, loop, occurrence or pick that select which
/// occurrence of that segment, so two columns reading the same position of
/// differently qualified segments compare equal.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(
    name = "ElementSource",
    module = "oxedi",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyElementSource {
    segment: Vec<u8>,
    element: usize,
    component: Option<usize>,
}

impl PyElementSource {
    fn new(segment: &[u8], element: usize, component: Option<usize>) -> Self {
        PyElementSource {
            segment: segment.to_vec(),
            element,
            component,
        }
    }

    /// The source of a column that reads one fixed element; `None` for any
    /// other kind of column.
    fn of(source: &ColumnSource) -> Option<Self> {
        let ColumnSource::Element {
            segment,
            element,
            component,
            ..
        } = source
        else {
            return None;
        };
        Some(Self::new(segment, *element, *component))
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PyElementSource {
    /// The id of the segment the column reads, such as `b"CLP"`; for a named
    /// occurrence, the occurrence's segment; in a table with `segment`, that
    /// segment.
    #[getter]
    fn segment<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.segment)
    }

    /// The 1-based element position.
    #[getter]
    fn element(&self) -> usize {
        self.element
    }

    /// The 1-based component position, when the column reads one component of
    /// a composite.
    #[getter]
    fn component(&self) -> Option<usize> {
        self.component
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let segment = PyBytes::new(py, &self.segment).repr()?;
        let component = self
            .component
            .map_or_else(|| "None".to_owned(), |position| position.to_string());
        Ok(format!(
            "ElementSource(segment={segment}, element={}, component={component})",
            self.element
        ))
    }
}

/// Where a table column reads its value in a segment that repeats a group of
/// elements, one row per group: the element at `offset` inside each group, or
/// one component of it. `holds(position)` says whether an element position of
/// the segment is that element of some group.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(
    name = "GroupSource",
    module = "oxedi",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyGroupSource {
    first: usize,
    step: usize,
    offset: usize,
    component: Option<usize>,
}

impl PyGroupSource {
    /// The source of a column that reads an element of each group of the
    /// table's repeat; `None` for any other kind of column.
    fn of(def: &TableDef, source: &ColumnSource) -> Option<Self> {
        let ColumnSource::GroupElement { offset, component } = source else {
            return None;
        };
        let repeat = def.repeat?;
        Some(PyGroupSource {
            first: repeat.from,
            step: repeat.step,
            offset: *offset,
            component: *component,
        })
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PyGroupSource {
    /// The 1-based position of the first group's first element.
    #[getter]
    fn first(&self) -> usize {
        self.first
    }

    /// Elements per group.
    #[getter]
    fn step(&self) -> usize {
        self.step
    }

    /// The 0-based position of the column's element inside each group.
    #[getter]
    fn offset(&self) -> usize {
        self.offset
    }

    /// The 1-based component position, when the column reads one component of
    /// a composite.
    #[getter]
    fn component(&self) -> Option<usize> {
        self.component
    }

    /// Whether 1-based element `position` of the segment is the column's
    /// element in some group.
    fn holds(&self, position: usize) -> bool {
        Repeat {
            from: self.first,
            step: self.step,
        }
        .holds(self.offset, position)
    }

    fn __repr__(&self) -> String {
        let component = self
            .component
            .map_or_else(|| "None".to_owned(), |position| position.to_string());
        format!(
            "GroupSource(first={}, step={}, offset={}, component={component})",
            self.first, self.step, self.offset
        )
    }
}

/// The `KeyError` of a refused lookup: the core's error, after the call it
/// came from.
fn refusal(table: &str, column: &str, unknown: impl std::fmt::Display) -> PyErr {
    PyKeyError::new_err(format!(
        "Spec.column_source({table:?}, {column:?}): {unknown}"
    ))
}

/// Where column `column` of table `table` reads its value: `KeyError` for a
/// table or column the spec does not declare, `ValueError` for a column with
/// no single element to name.
pub fn resolve(spec: &Spec, table: &str, column: &str) -> PyResult<PyElementSource> {
    let def = spec
        .require_table(table)
        .map_err(|unknown| refusal(table, column, unknown))?;
    let source = def
        .require_column(column)
        .map_err(|unknown| refusal(table, column, unknown))?;
    match source {
        ColumnSource::Element {
            segment,
            element,
            component,
            ..
        } => Ok(PyElementSource::new(segment, *element, *component)),
        ColumnSource::SegmentIndex { segment, .. } => Err(PyValueError::new_err(format!(
            "Spec.column_source({table:?}, {column:?}): column {column:?} of table {table:?} holds the index of a {} segment, not an element; ask for a column that reads an element",
            String::from_utf8_lossy(segment)
        ))),
        ColumnSource::GroupElement { offset, .. } => Err(PyValueError::new_err(format!(
            "Spec.column_source({table:?}, {column:?}): column {column:?} of table {table:?} reads offset {offset} of each repeated element group, so its element changes from row to row; ask for a column that reads one fixed element"
        ))),
    }
}

/// The columns of table `table` that `read` gives a source for, by name, in
/// name order; `KeyError` naming call `call` for a table the spec lacks.
fn sources<T>(
    spec: &Spec,
    call: &str,
    table: &str,
    read: impl Fn(&TableDef, &ColumnSource) -> Option<T>,
) -> PyResult<BTreeMap<String, T>> {
    let def = spec
        .require_table(table)
        .map_err(|unknown| PyKeyError::new_err(format!("Spec.{call}({table:?}): {unknown}")))?;
    Ok(def
        .columns
        .iter()
        .filter_map(|(name, source)| read(def, source).map(|found| (name.clone(), found)))
        .collect())
}

/// Every column of table `table` that reads one fixed element, by name.
pub fn element_sources(spec: &Spec, table: &str) -> PyResult<BTreeMap<String, PyElementSource>> {
    sources(spec, "element_sources", table, |_, source| {
        PyElementSource::of(source)
    })
}

/// Every column of table `table` that reads an element of each repeated
/// group, by name.
pub fn group_sources(spec: &Spec, table: &str) -> PyResult<BTreeMap<String, PyGroupSource>> {
    sources(spec, "group_sources", table, PyGroupSource::of)
}
