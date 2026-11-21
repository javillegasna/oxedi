//! The schema of the tables a spec projects: each table's columns, their
//! types and the tables their reference columns point at, known from the
//! spec alone.

use std::fmt;

use super::{ColumnSource, ROW_COLUMN, SEGMENT_COLUMN, Spec, TableDef, UnknownColumn};
use crate::value::{ColumnType, ElementType};

/// What a column reads, in the spec's own terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnKind {
    /// The index of a segment.
    SegmentIndex,
    /// An element or component; `None` when the spec does not define it.
    Element(Option<ElementType>),
}

impl fmt::Display for ColumnKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColumnKind::SegmentIndex => write!(f, "a segment index"),
            ColumnKind::Element(Some(kind)) => write!(f, "{kind}"),
            ColumnKind::Element(None) => write!(f, "an element the spec does not define"),
        }
    }
}

impl Spec {
    /// What `source` of a column of `table` reads: a segment index, or the
    /// element type the spec declares for the element (`None` when it
    /// declares none).
    pub(crate) fn column_kind(&self, table: &TableDef, source: &ColumnSource) -> ColumnKind {
        let def = match source {
            ColumnSource::Element {
                segment,
                element,
                component,
                ..
            } => self.element_def(segment, *element, *component),
            ColumnSource::SegmentIndex { .. } => return ColumnKind::SegmentIndex,
            ColumnSource::GroupElement { offset, component } => {
                match (&table.segment, table.repeat) {
                    (Some(segment), Some(repeat)) => repeat
                        .from
                        .checked_add(*offset)
                        .and_then(|position| self.element_def(segment, position, *component)),
                    _ => None,
                }
            }
        };
        ColumnKind::Element(def.map(|def| def.kind))
    }
}
/// One column of a projected table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnSchema {
    /// The column's name.
    pub name: String,
    /// The type of its values.
    pub kind: ColumnType,
    /// For a reference column, the table whose `row` it holds; `None`
    /// for every other column.
    pub parent: Option<String>,
}

/// The columns of one projected table, in the order the projector fills
/// them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSchema {
    /// The table's name.
    pub name: String,
    /// Every column, in order.
    pub columns: Vec<ColumnSchema>,
}

/// The column type of a declared column: its element's type, `Int64` for a
/// segment index, `Binary` for an element the spec does not define.
pub(crate) fn column_type(spec: &Spec, table: &TableDef, source: &ColumnSource) -> ColumnType {
    match spec.column_kind(table, source) {
        ColumnKind::SegmentIndex => ColumnType::Int64 { scale: 0 },
        ColumnKind::Element(kind) => ColumnType::of(kind),
    }
}

/// The schema of one table of `spec`: the row number, the anchor segment's
/// index, a reference per table above (outermost first, each naming that
/// table as its parent), then the declared columns in name order.
pub fn table_schema(spec: &Spec, table: &TableDef) -> TableSchema {
    let index_column = ColumnType::Int64 { scale: 0 };
    let plain = |name: &str, kind| ColumnSchema {
        name: name.to_string(),
        kind,
        parent: None,
    };
    let mut columns = vec![
        plain(ROW_COLUMN, index_column),
        plain(SEGMENT_COLUMN, index_column),
    ];
    for &above in &table.ancestors {
        if let Some(def) = spec.tables().get(above) {
            columns.push(ColumnSchema {
                name: def.reference.clone(),
                kind: index_column,
                parent: Some(def.name.clone()),
            });
        }
    }
    columns.extend(
        table
            .columns
            .iter()
            .map(|(name, source)| plain(name, column_type(spec, table, source))),
    );
    TableSchema {
        name: table.name.clone(),
        columns,
    }
}

/// The schema of every table of `spec`, in the spec's table order.
pub fn spec_schema(spec: &Spec) -> Vec<TableSchema> {
    spec.tables()
        .iter()
        .map(|table| table_schema(spec, table))
        .collect()
}

/// Every column of a table the projector fills, with its type, in order;
/// see [`table_schema`].
pub fn table_columns(spec: &Spec, table: &TableDef) -> Vec<(String, ColumnType)> {
    table_schema(spec, table)
        .columns
        .into_iter()
        .map(|column| (column.name, column.kind))
        .collect()
}

/// The 0-based position and the type of column `name` in a table's schema,
/// as [`table_columns`] returns it; a name the schema does not hold is an
/// [`UnknownColumn`] that lists every column of `columns`, in order.
pub fn require_column(
    table: &str,
    columns: &[(String, ColumnType)],
    name: &str,
) -> Result<(usize, ColumnType), UnknownColumn> {
    columns
        .iter()
        .enumerate()
        .find_map(|(at, (column, kind))| (column == name).then_some((at, *kind)))
        .ok_or_else(|| UnknownColumn {
            table: table.to_owned(),
            column: name.to_owned(),
            columns: columns.iter().map(|(column, _)| column.clone()).collect(),
        })
}
