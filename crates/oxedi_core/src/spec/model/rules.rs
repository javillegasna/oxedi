//! The version declaration, the balancing rules and the operations of a
//! summary, as written.

use serde::{Deserialize, Serialize};

use super::declare::spec_object;
use crate::spec::Sign;
use crate::spec::sections::{COUNT, COUNTS, TEXT, TEXTS, nullable, objects};
use crate::spec::shape::{Field, Leaf};

spec_object! {
    /// The element of a segment that declares the version a file is written in,
    /// and the values the spec accepts there.
    VersionModel = RawVersion, VERSION, "a version object";
    /// The segment that holds the declaration.
    segment: String, TEXT, required;
    /// 1-based element position of the declaration.
    element: usize, COUNT, required;
    /// The values the spec accepts.
    values: Vec<String>, TEXTS, required;
}

spec_object! {
    /// A summary over the rows of a table, written `{"count": {"table": ...}}`.
    SummaryTableModel = RawSummaryTable, SUMMARY_TABLE, "a summary operation object";
    /// The table.
    table: String, TEXT, required;
}

spec_object! {
    /// A summary over one column of a table.
    SummaryColumnModel = RawSummaryColumn, SUMMARY_COLUMN, "a summary operation object";
    /// The table.
    table: String, TEXT, required;
    /// The column.
    column: String, TEXT, required;
}

spec_object! {
    /// A summary of named fields of the one row of a table.
    SummaryRowModel = RawSummaryRow, SUMMARY_ROW, "a summary operation object";
    /// The table.
    table: String, TEXT, required;
    /// The fields, in the order written.
    fields: Vec<SummaryFieldModel>, objects!(&SUMMARY_FIELD), required;
}

spec_object! {
    /// One field of a row summary.
    SummaryFieldModel = RawSummaryField, SUMMARY_FIELD, "a summary field object";
    /// The field's name in the summary.
    name: String, TEXT, required;
    /// The column it reads.
    column: String, TEXT, required;
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

spec_object! {
    /// Amounts that must add up inside each instance of a loop.
    BalanceModel / RawBalance, BALANCE, "a balancing rule object";
    /// The loop whose instances are checked.
    per: String, TEXT, required;
    /// The values the sum must equal, added together.
    target: Vec<TermModel> | Vec<RawTerm>, objects!(&TERM), required;
    /// The values added together.
    sum: Vec<TermModel> | Vec<RawTerm>, objects!(&TERM), required;
}

spec_object! {
    /// One value of a balancing rule: elements of an occurrence, added with a sign.
    TermModel / RawTerm, TERM, "a balancing value object";
    /// The loop that holds the occurrence, when not the rule's `per` loop.
    loop_name as "loop": Option<String>, nullable(Leaf::Text), optional;
    /// The occurrence.
    occurrence: String, TEXT, required;
    /// 1-based component position inside each element, when the value is one.
    component: Option<usize>, nullable(Leaf::Count), optional;
    /// Whether the value is added or subtracted.
    sign: Sign | Option<String>, nullable(Leaf::Text), default_or_null;
    /// 1-based element positions whose values are added.
    elements: Vec<usize>, COUNTS, required;
}
