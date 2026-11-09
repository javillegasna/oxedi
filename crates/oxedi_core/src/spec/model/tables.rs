//! The tables of a spec, as the spec writes them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::is_default;
use crate::mask::Phi;
use crate::spec::Pick;

/// A table: the loops its rows come from and the columns each row holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a table object")]
pub struct TableModel {
    /// The loops whose instances become rows, outermost first.
    pub loops: Vec<String>,
    /// The name of the column that refers to a row of this table from the
    /// tables below it, when not the table's name.
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// The segment each of whose occurrences is a row, when rows are not
    /// loop instances.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment: Option<String>,
    /// The repeated elements of the anchor segment, when each is a row.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat: Option<RepeatModel>,
    /// The columns, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub columns: BTreeMap<String, ColumnModel>,
    /// The column whose value seeds the date shift of the row, when one does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phi_seed: Option<String>,
}

/// One column of a table: where its value is read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a column object")]
pub struct ColumnModel {
    /// The loop it reads, when not the table's anchor loop.
    #[serde(rename = "loop", skip_serializing_if = "Option::is_none")]
    pub loop_name: Option<String>,
    /// The segment it reads, when it names a segment instead of an occurrence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment: Option<String>,
    /// The value each element of the segment must hold, by 1-based element
    /// position.
    #[serde(default, rename = "where", skip_serializing_if = "BTreeMap::is_empty")]
    pub conditions: BTreeMap<usize, String>,
    /// The occurrence it reads, when it names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occurrence: Option<String>,
    /// Which of the matching segments it reads, when the spec writes it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pick: Option<Pick>,
    /// 1-based element position, when it reads an element.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<usize>,
    /// 1-based component position inside the element, when it reads one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub component: Option<usize>,
    /// 0-based position inside the element group, when the table repeats.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_element: Option<usize>,
    /// Whether it holds the index of the segment it reads.
    #[serde(default, skip_serializing_if = "is_default")]
    pub segment_index: bool,
    /// What patient data it holds, when it holds any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phi: Option<Phi>,
}

/// Where the repeated elements of a table's segment start and how far apart
/// they are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a repeat object")]
pub struct RepeatModel {
    /// 1-based element position of the first repetition.
    pub from: usize,
    /// Elements between one repetition and the next.
    pub step: usize,
}
