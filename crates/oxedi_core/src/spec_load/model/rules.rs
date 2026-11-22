//! The version declaration, the balancing rules and the operations of a
//! summary, as written.

use serde::{Deserialize, Serialize};

use super::declare::spec_object;
use super::present;
use crate::spec::Sign;
use crate::spec_load::SummaryDefError;

spec_object! {
    /// The element of a segment that declares the version a file is written in,
    /// and the values the spec accepts there.
    VersionModel, "a version object";
    /// The segment that holds the declaration.
    segment: String, required;
    /// 1-based element position of the declaration.
    element: usize, required;
    /// The values the spec accepts.
    values: Vec<String>, required;
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
#[serde(rename_all = "lowercase", try_from = "SummaryKeys")]
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

// The keys a summary object may hold; exactly one must be present, which
// the conversion checks so the error lists the ones held. A key written
// `null` is refused, not read as absent.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, expecting = "a summary object")]
struct SummaryKeys {
    #[serde(default, deserialize_with = "present")]
    count: Option<SummaryTableModel>,
    #[serde(default, deserialize_with = "present")]
    distinct: Option<SummaryColumnModel>,
    #[serde(default, deserialize_with = "present")]
    row: Option<SummaryRowModel>,
    #[serde(default, deserialize_with = "present")]
    sum: Option<SummaryColumnModel>,
}

impl TryFrom<SummaryKeys> for SummaryModel {
    type Error = SummaryDefError;

    fn try_from(keys: SummaryKeys) -> Result<SummaryModel, SummaryDefError> {
        let mut held: Vec<(&str, SummaryModel)> = [
            ("count", keys.count.map(SummaryModel::Count)),
            ("distinct", keys.distinct.map(SummaryModel::Distinct)),
            ("row", keys.row.map(SummaryModel::Row)),
            ("sum", keys.sum.map(SummaryModel::Sum)),
        ]
        .into_iter()
        .filter_map(|(key, operation)| operation.map(|operation| (key, operation)))
        .collect();
        match held.pop() {
            Some((_, operation)) if held.is_empty() => Ok(operation),
            last => {
                held.extend(last);
                Err(SummaryDefError::Operations {
                    found: held.iter().map(|(key, _)| key.to_string()).collect(),
                })
            }
        }
    }
}

spec_object! {
    /// Amounts that must add up inside each instance of a loop.
    BalanceModel, "a balancing rule object";
    /// The loop whose instances are checked.
    per: String, required;
    /// The values the sum must equal, added together.
    target: Vec<TermModel>, required;
    /// The values added together.
    sum: Vec<TermModel>, required;
}

spec_object! {
    /// One value of a balancing rule: elements of an occurrence, added with a sign.
    TermModel, "a balancing value object";
    /// The loop that holds the occurrence, when not the rule's `per` loop.
    loop_name as "loop": Option<String>, optional;
    /// The occurrence.
    occurrence: String, required;
    /// 1-based component position inside each element, when the value is one.
    component: Option<usize>, optional;
    /// Whether the value is added or subtracted.
    sign: Sign, default_or_null;
    /// 1-based element positions whose values are added.
    elements: Vec<usize>, required;
}
