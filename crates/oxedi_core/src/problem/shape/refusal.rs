//! The shape of a write refusal.

use super::{Place, Problem, SpecPlace};
use crate::write::Refusal;

problem!(Refusal [RequiredLoopWithoutSource, RequiredOccurrenceWithoutSource, RequiredElementWithoutSource, AmbiguousSource, PickNotContiguous, WrittenByAnotherTable, DuplicateSource, EnvelopeColumn, EnvelopeWithoutOccurrence, AnchoredByAnotherTable, SeveralAnchorLoops, RepeatWithoutInstances, CodeOutsideList] this => {
        let table = |name, column| Place::Spec(SpecPlace::Table { name, column });
        let in_loop = |name, occurrence| Place::Spec(SpecPlace::Loop { name, occurrence });
        match this {
            Refusal::RequiredLoopWithoutSource { loop_name, .. }
            | Refusal::EnvelopeWithoutOccurrence { loop_name, .. } => in_loop(loop_name, None),
            Refusal::RequiredOccurrenceWithoutSource {
                loop_name,
                occurrence,
                ..
            }
            | Refusal::RequiredElementWithoutSource {
                table: None,
                loop_name,
                occurrence,
                ..
            } => in_loop(loop_name, Some(occurrence)),
            Refusal::RequiredElementWithoutSource {
                table: Some(name), ..
            } => table(name, None),
            Refusal::AmbiguousSource {
                table: name,
                column,
                ..
            }
            | Refusal::CodeOutsideList {
                table: name,
                column,
                ..
            } => table(name, column.as_deref()),
            Refusal::PickNotContiguous {
                table: name,
                column,
                ..
            }
            | Refusal::WrittenByAnotherTable {
                table: name,
                column,
                ..
            }
            | Refusal::DuplicateSource {
                table: name,
                column,
                ..
            }
            | Refusal::EnvelopeColumn {
                table: name,
                column,
                ..
            } => table(name, Some(column)),
            Refusal::AnchoredByAnotherTable { table: name, .. }
            | Refusal::SeveralAnchorLoops { table: name, .. }
            | Refusal::RepeatWithoutInstances { table: name, .. } => table(name, None),
        }
});
