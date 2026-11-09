//! The version declaration and the operations of a summary, as the spec
//! writes them.

use serde::{Deserialize, Serialize};

/// The element of a segment that declares the version a file is written in,
/// and the values the spec accepts there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a version object")]
pub struct VersionModel {
    /// The segment that holds the declaration.
    pub segment: String,
    /// 1-based element position of the declaration.
    pub element: usize,
    /// The values the spec accepts.
    pub values: Vec<String>,
}

/// A summary over the rows of a table, written `{"count": {"table": ...}}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a summary operation object")]
pub struct SummaryTableModel {
    /// The table.
    pub table: String,
}

/// A summary over one column of a table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a summary operation object")]
pub struct SummaryColumnModel {
    /// The table.
    pub table: String,
    /// The column.
    pub column: String,
}

/// A summary of named fields of the one row of a table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a summary operation object")]
pub struct SummaryRowModel {
    /// The table.
    pub table: String,
    /// The fields, in the order written.
    pub fields: Vec<SummaryFieldModel>,
}

/// One field of a row summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a summary field object")]
pub struct SummaryFieldModel {
    /// The field's name in the summary.
    pub name: String,
    /// The column it reads.
    pub column: String,
}

/// The one operation a summary holds, the only key of its object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SummaryModel {
    /// The number of rows of a table.
    Count(SummaryTableModel),
    /// The number of distinct non-null values of a column.
    Distinct(SummaryColumnModel),
    /// The sum of a decimal column.
    Sum(SummaryColumnModel),
    /// Named fields of the one row of a table.
    Row(SummaryRowModel),
}
