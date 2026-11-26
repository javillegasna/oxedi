//! The summaries a spec declares, read for `Result.summary`: the core
//! computes them, this module turns the value and the errors into Python's.

use oxedi_core::value::decimal_display;
use oxedi_core::{SummaryError, SummaryValue, summarize};
use pyo3::exceptions::{PyKeyError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::tables::PyTables;

/// The fields of a row summary by name, each the cell's text or `None` for
/// a null cell; `None` as a whole when every field is null.
type Row = Option<Vec<(String, Option<String>)>>;

/// The summary `name`, computed over the tables.
fn summary(tables: &PyTables, name: &str) -> PyResult<SummaryValue> {
    summarize(tables.spec(), name, tables.tables()).map_err(|error| {
        let text = format!("Result.summary({name:?}): {error}");
        match error {
            SummaryError::Undeclared { .. }
            | SummaryError::MissingTable { .. }
            | SummaryError::MissingColumn { .. } => PyKeyError::new_err(text),
            SummaryError::NotDecimal { .. } => PyTypeError::new_err(text),
            SummaryError::NotOneRow { .. } | SummaryError::Overflow { .. } => {
                PyValueError::new_err(text)
            }
        }
    })
}

/// A `count` or `distinct` summary as an `int`.
fn count_value(py: Python<'_>, count: usize) -> PyResult<Bound<'_, PyAny>> {
    Ok(count.into_pyobject(py)?.into_any())
}

/// A `sum` summary as a `decimal.Decimal` at its column's scale.
fn decimal<'py>(py: Python<'py>, value: i128, scale: u8) -> PyResult<Bound<'py, PyAny>> {
    py.import("decimal")?
        .getattr("Decimal")?
        .call1((decimal_display(value, scale),))
}

/// A `row` summary as a dict of its fields in the order the spec writes them,
/// or `None` for a row the file holds nothing of.
fn row<'py>(py: Python<'py>, fields: Row) -> PyResult<Bound<'py, PyAny>> {
    let Some(fields) = fields else {
        return Ok(py.None().into_bound(py));
    };
    let dict = PyDict::new(py);
    fields
        .into_iter()
        .try_for_each(|(field, cell)| dict.set_item(field, cell))?;
    Ok(dict.into_any())
}

/// The summary `name` as the Python value of the kind the spec declares: an
/// `int` for a count, a `Decimal` for a sum, a dict or `None` for a row.
pub fn value<'py>(py: Python<'py>, tables: &PyTables, name: &str) -> PyResult<Bound<'py, PyAny>> {
    match summary(tables, name)? {
        SummaryValue::Count(count) => count_value(py, count),
        SummaryValue::Decimal { value, scale } => decimal(py, value, scale),
        SummaryValue::Row(fields) => row(py, fields),
    }
}
