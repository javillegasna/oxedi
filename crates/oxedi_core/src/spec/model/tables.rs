//! The tables of a spec, as the spec writes them.

use std::collections::BTreeMap;

use serde::Deserializer;

use super::declare::spec_object;
use super::positions;
use crate::spec::Pick;
use crate::spec::sections::{COUNT, TEXT, TEXTS, leaf, nullable};
use crate::spec::shape::{Field, Leaf};
use crate::vocabulary::Phi;

spec_object! {
    /// A table: the loops its rows come from and the columns each row holds.
    TableModel, TABLE, "a table object";
    /// The loops whose instances become rows, outermost first.
    loops: Vec<String>, TEXTS, required;
    /// The name of the column that refers to a row of this table from the
    /// tables below it, when not the table's name.
    reference as "ref": Option<String>, nullable(Leaf::Text), optional;
    /// The segment each of whose occurrences is a row, when rows are not
    /// loop instances.
    segment: Option<String>, nullable(Leaf::Text), optional;
    /// The column whose value seeds the date shift of the row, when one does.
    phi_seed: Option<String>, nullable(Leaf::Text), optional;
    /// The repeated elements of the anchor segment, when each is a row.
    repeat: Option<RepeatModel>, Field::Object(&REPEAT), optional;
    /// The columns, by name.
    columns: BTreeMap<String, ColumnModel>, Field::Map(&Field::Object(&COLUMN)), default;
}

spec_object! {
    /// One column of a table: where its value is read.
    ColumnModel, COLUMN, "a column object";
    /// The loop it reads, when not the table's anchor loop.
    loop_name as "loop": Option<String>, nullable(Leaf::Text), optional;
    /// The segment it reads, when it names a segment instead of an occurrence.
    segment: Option<String>, nullable(Leaf::Text), optional;
    /// The value each element of the segment must hold, by 1-based element
    /// position.
    #[serde(deserialize_with = "column_where")]
    conditions as "where": BTreeMap<usize, String>, Field::Map(&TEXT), default;
    /// The occurrence it reads, when it names one.
    occurrence: Option<String>, nullable(Leaf::Text), optional;
    /// Which of the matching segments it reads, when the spec writes it.
    pick: Option<Pick>, nullable(Leaf::Pick), optional;
    /// 1-based element position, when it reads an element.
    element: Option<usize>, nullable(Leaf::Count), optional;
    /// 1-based component position inside the element, when it reads one.
    component: Option<usize>, nullable(Leaf::Count), optional;
    /// 0-based position inside the element group, when the table repeats.
    group_element: Option<usize>, nullable(Leaf::Count), optional;
    /// Whether it holds the index of the segment it reads.
    segment_index: bool, leaf(Leaf::Flag), default;
    /// What patient data it holds, when it holds any.
    phi: Option<Phi>, nullable(Leaf::Text), optional;
}

fn column_where<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<usize, String>, D::Error> {
    positions(deserializer, "column \"where\"")
}

spec_object! {
    /// Where the repeated elements of a table's segment start and how far apart
    /// they are.
    RepeatModel, REPEAT, "a repeat object";
    /// 1-based element position of the first repetition.
    from: usize, COUNT, required;
    /// Elements between one repetition and the next.
    step: usize, COUNT, required;
}
