//! The caller's tables bound to the spec's: each spec table's columns found
//! by name and checked against the types the spec gives them.

use crate::column::{Cell, ColumnData, Table, Tables};
use crate::spec::{ROW_COLUMN, SEGMENT_COLUMN, Spec, TableDef, UnknownColumn, table_columns};
use crate::value::ColumnType;

use super::finding::WriteError;

/// One spec table's data: absent tables have no rows, absent columns are
/// null.
#[derive(Debug, Clone)]
pub(super) struct Data<'t> {
    /// The table's name.
    pub(super) name: String,
    /// Its rows.
    pub(super) rows: usize,
    /// The row number column.
    pub(super) row: Option<&'t ColumnData>,
    /// The anchor segment's index column.
    pub(super) segment: Option<&'t ColumnData>,
    /// The reference to each table above, by that table's index.
    pub(super) refs: Vec<(usize, Option<&'t ColumnData>)>,
    /// The declared columns, by index into the spec's columns.
    pub(super) columns: Vec<Option<&'t ColumnData>>,
}

impl<'t> Data<'t> {
    /// The cell of a declared column.
    pub(super) fn cell(&self, column: usize, row: usize) -> Cell<'t> {
        self.columns
            .get(column)
            .copied()
            .flatten()
            .and_then(|data| data.get(row))
            .unwrap_or(Cell::Null)
    }

    /// A declared column, when the caller gave it.
    pub(super) fn column(&self, column: usize) -> Option<&'t ColumnData> {
        self.columns.get(column).copied().flatten()
    }

    /// The reference to table `above` of a row; `None` when null or absent.
    pub(super) fn reference(&self, above: usize, row: usize) -> Option<i64> {
        let data = self
            .refs
            .iter()
            .find(|(table, _)| *table == above)
            .and_then(|(_, data)| *data)?;
        match data.get(row)? {
            Cell::Int64(value) => Some(value),
            _ => None,
        }
    }

    /// The row number of a row: its row number column, or its position
    /// when that is null or absent.
    pub(super) fn key(&self, row: usize) -> i64 {
        match self.row.and_then(|data| data.get(row)) {
            Some(Cell::Int64(value)) => value,
            _ => i64::try_from(row).unwrap_or(i64::MAX),
        }
    }
}

/// Binds every spec table to the caller's table of the same name.
pub(super) fn bind<'t>(spec: &Spec, tables: &'t Tables) -> Result<Vec<Data<'t>>, WriteError> {
    for table in tables {
        spec.require_table(table.name())?;
    }
    spec.tables()
        .iter()
        .map(|def| bind_table(spec, def, tables.get(&def.name)))
        .collect()
}

/// One spec table bound to the caller's table of that name, when given.
fn bind_table<'t>(
    spec: &Spec,
    def: &TableDef,
    given: Option<&'t Table>,
) -> Result<Data<'t>, WriteError> {
    let schema = table_columns(spec, def);
    let mut found: Vec<Option<&ColumnData>> = vec![None; schema.len()];
    for (name, data) in given.map_or(&[][..], |table| table.columns()) {
        let at = locate(def, &schema, name, data)?;
        if let Some(slot) = found.get_mut(at) {
            *slot = Some(data);
        }
    }
    let by_name = |name: &str| {
        schema
            .iter()
            .position(|(column, _)| column == name)
            .and_then(|at| found.get(at).copied().flatten())
    };
    Ok(Data {
        name: def.name.clone(),
        rows: given.map_or(0, |table| table.len()),
        row: by_name(ROW_COLUMN),
        segment: by_name(SEGMENT_COLUMN),
        refs: def
            .ancestors
            .iter()
            .map(|&above| {
                let reference = spec
                    .tables()
                    .get(above)
                    .map(|table| table.reference.as_str())
                    .unwrap_or_default();
                (above, by_name(reference))
            })
            .collect(),
        columns: def.columns.iter().map(|(name, _)| by_name(name)).collect(),
    })
}

/// The position in `schema` of the caller's column `name`, refused when the
/// table has no such column or the column's type is not the spec's.
fn locate(
    def: &TableDef,
    schema: &[(String, ColumnType)],
    name: &str,
    data: &ColumnData,
) -> Result<usize, WriteError> {
    let (at, (_, expected)) = schema
        .iter()
        .enumerate()
        .find(|(_, (column, _))| column == name)
        .ok_or_else(|| {
            let columns = schema.iter().map(|(column, _)| column.clone()).collect();
            UnknownColumn {
                table: def.name.clone(),
                column: name.to_owned(),
                columns,
            }
        })?;
    if data.kind() != *expected {
        return Err(WriteError::ColumnType {
            table: def.name.clone(),
            column: name.to_owned(),
            expected: *expected,
            found: data.kind(),
        });
    }
    Ok(at)
}
