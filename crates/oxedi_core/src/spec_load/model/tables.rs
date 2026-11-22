//! The tables of a spec, as the spec writes them.

use std::collections::BTreeMap;

use serde::Deserializer;

use super::declare::spec_object;
use super::{positions, present};
use crate::vocabulary::{Phi, Pick};

spec_object! {
    /// A table: the loops its rows come from and the columns each row holds.
    TableModel, "a table object";
    /// The loops whose instances become rows, outermost first.
    loops: Vec<String>, required;
    /// The name of the column that refers to a row of this table from the
    /// tables below it, when not the table's name.
    reference as "ref": Option<String>, optional;
    /// The segment each of whose occurrences is a row, when rows are not
    /// loop instances.
    segment: Option<String>, optional;
    /// The column whose value seeds the date shift of the row, when one does.
    phi_seed: Option<String>, optional;
    /// The repeated elements of the anchor segment, when each is a row.
    #[serde(default, deserialize_with = "present")]
    repeat: Option<RepeatModel>, optional;
    /// The columns, by name.
    columns: BTreeMap<String, ColumnModel>, default;
}

spec_object! {
    /// One column of a table: where its value is read.
    ColumnModel, "a column object";
    /// The loop it reads, when not the table's anchor loop.
    loop_name as "loop": Option<String>, optional;
    /// The segment it reads, when it names a segment instead of an occurrence.
    segment: Option<String>, optional;
    /// The value each element of the segment must hold, by 1-based element
    /// position.
    #[serde(deserialize_with = "column_where")]
    conditions as "where": BTreeMap<usize, String>, default;
    /// The occurrence it reads, when it names one.
    occurrence: Option<String>, optional;
    /// Which of the matching segments it reads, when the spec writes it.
    pick: Option<Pick>, optional;
    /// 1-based element position, when it reads an element.
    element: Option<usize>, optional;
    /// 1-based component position inside the element, when it reads one.
    component: Option<usize>, optional;
    /// 0-based position inside the element group, when the table repeats.
    group_element: Option<usize>, optional;
    /// Whether it holds the index of the segment it reads.
    segment_index: bool, default;
    /// What patient data it holds, when it holds any.
    phi: Option<Phi>, optional;
}

fn column_where<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<usize, String>, D::Error> {
    positions(deserializer, "column \"where\"")
}

spec_object! {
    /// Where the repeated elements of a table's segment start and how far apart
    /// they are.
    RepeatModel, "a repeat object";
    /// 1-based element position of the first repetition.
    from: usize, required;
    /// Elements between one repetition and the next.
    step: usize, required;
}
