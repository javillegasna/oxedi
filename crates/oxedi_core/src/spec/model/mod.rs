//! The spec as typed data, read from and written to its JSON, so a program
//! builds and serializes it without a JSON string. `loops` holds the loops
//! and their occurrences, `segments` the segments and their elements,
//! `tables` the tables and their columns, and `rules` the version
//! declaration, the balancing rules and the operations of a summary.

mod loops;
mod rules;
mod segments;
mod tables;

use serde::{Deserialize, Deserializer};

pub use loops::{ControlModel, LoopModel, OccurrenceModel, QualifierModel, TriggerModel};
pub use rules::{
    BalanceModel, SummaryColumnModel, SummaryFieldModel, SummaryModel, SummaryRowModel,
    SummaryTableModel, TermModel, VersionModel,
};
pub use segments::{ElementModel, SegmentModel};
pub use tables::{ColumnModel, RepeatModel, TableModel};

// A key equal to its default is not written.
fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

// A key written `null` reads as its default, as an absent key does.
fn null_is_default<'de, D: Deserializer<'de>, T: Deserialize<'de> + Default>(
    deserializer: D,
) -> Result<T, D::Error> {
    Option::<T>::deserialize(deserializer).map(Option::unwrap_or_default)
}
