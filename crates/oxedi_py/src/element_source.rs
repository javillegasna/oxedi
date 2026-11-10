//! `ElementSource`: where a table column reads its value, as Python attributes.

use oxedi_core::{ColumnSource, Spec, TableDef};
use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};

/// Where a table column reads its value: one element, or one component of it,
/// of a segment.
#[gen_stub_pyclass(module = "oxedi._core")]
#[pyclass(
    name = "ElementSource",
    module = "oxedi",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyElementSource {
    segment: Vec<u8>,
    element: usize,
    component: Option<usize>,
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

/// The names, quoted and joined, a refusal offers.
fn names<'n>(names: impl Iterator<Item = &'n str>) -> String {
    names
        .map(|name| format!("{name:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn table_of<'s>(spec: &'s Spec, table: &str, column: &str) -> PyResult<&'s TableDef> {
    spec.table(table).ok_or_else(|| {
        PyKeyError::new_err(format!(
            "Spec.column_source({table:?}, {column:?}): the spec defines no table {table:?}; pass one of its tables: {}",
            names(spec.tables().iter().map(|def| def.name.as_str()))
        ))
    })
}

fn source_of<'s>(def: &'s TableDef, table: &str, column: &str) -> PyResult<&'s ColumnSource> {
    def.column(column).ok_or_else(|| {
        PyKeyError::new_err(format!(
            "Spec.column_source({table:?}, {column:?}): table {table:?} declares no column {column:?}; pass one of its columns: {}",
            names(def.columns.iter().map(|(name, _)| name.as_str()))
        ))
    })
}

/// Where column `column` of table `table` reads its value: `KeyError` for a
/// table or column the spec does not declare, `ValueError` for a column with
/// no single element to name.
pub fn resolve(spec: &Spec, table: &str, column: &str) -> PyResult<PyElementSource> {
    let def = table_of(spec, table, column)?;
    match source_of(def, table, column)? {
        ColumnSource::Element {
            segment,
            element,
            component,
            ..
        } => Ok(PyElementSource {
            segment: segment.clone(),
            element: *element,
            component: *component,
        }),
        ColumnSource::SegmentIndex { segment, .. } => Err(PyValueError::new_err(format!(
            "Spec.column_source({table:?}, {column:?}): column {column:?} of table {table:?} holds the index of a {} segment, not an element; ask for a column that reads an element",
            String::from_utf8_lossy(segment)
        ))),
        ColumnSource::GroupElement { offset, .. } => Err(PyValueError::new_err(format!(
            "Spec.column_source({table:?}, {column:?}): column {column:?} of table {table:?} reads offset {offset} of each repeated element group, so its element changes from row to row; ask for a column that reads one fixed element"
        ))),
    }
}
