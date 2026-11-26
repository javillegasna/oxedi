//! The text of why a spec cannot be written, and what to change about it.

use std::fmt;

use crate::message::{Counted, render_names};
use crate::write::{PlanError, Refusal};

use super::wording::{Matches, Numbered, Source, WrittenBy};

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::RequiredLoopWithoutSource {
                loop_name,
                parent,
                trigger,
            } => write!(
                f,
                "loop {loop_name:?} (trigger {trigger}) is required in every {parent:?}, but no \
                 table is anchored on it and no column reads it"
            ),
            Refusal::RequiredOccurrenceWithoutSource {
                loop_name,
                occurrence,
                selector,
            } => write!(
                f,
                "occurrence {occurrence:?} ({selector}) is required in every loop \
                 {loop_name:?}, but no column writes it"
            ),
            Refusal::RequiredElementWithoutSource {
                table,
                loop_name,
                occurrence,
                place,
                name,
            } => write!(
                f,
                "required element {place} ({name}) of occurrence {occurrence:?} in loop \
                 {loop_name:?}{} has no column and no single code to write",
                WrittenBy(table.as_deref())
            ),
            Refusal::AmbiguousSource {
                table,
                column,
                loop_name,
                source,
                candidates,
            } => write!(
                f,
                "{} reads {source} in loop {loop_name:?}, which {}",
                Source(table, column.as_deref()),
                Matches(candidates)
            ),
            Refusal::PickNotContiguous {
                table,
                column,
                loop_name,
                occurrence,
                picks,
            } => write!(
                f,
                "table {table:?} column {column:?} picks occurrence {occurrence:?} of loop \
                 {loop_name:?} out of series (picks {}); the picks of one occurrence must run \
                 from 1 with none missing, or be a lone \"last\"",
                picks.join(", ")
            ),
            Refusal::WrittenByAnotherTable {
                table,
                column,
                loop_name,
                writer,
            } => write!(
                f,
                "table {table:?} column {column:?} reads loop {loop_name:?}, whose instances \
                 {writer} writes"
            ),
            Refusal::DuplicateSource {
                table,
                column,
                other,
                loop_name,
                occurrence,
                place,
            } => write!(
                f,
                "table {table:?} column {column:?} writes {place} of occurrence {occurrence:?} \
                 in loop {loop_name:?}, which column {other:?} already writes"
            ),
            Refusal::EnvelopeColumn {
                table,
                column,
                loop_name,
                occurrence,
            } => write!(
                f,
                "table {table:?} column {column:?} reads the envelope occurrence {occurrence:?} \
                 of loop {loop_name:?}, which the envelope and the writer's counts give"
            ),
            Refusal::EnvelopeWithoutOccurrence { loop_name, trigger } => write!(
                f,
                "the envelope loop {loop_name:?} declares no occurrence for its trigger {trigger}, \
                 so its header cannot be placed"
            ),
            Refusal::AnchoredByAnotherTable {
                table,
                loop_name,
                other,
            } => write!(
                f,
                "table {table:?} is anchored on loop {loop_name:?}, whose rows table {other:?} \
                 already gives; a loop takes its rows from one table"
            ),
            Refusal::SeveralAnchorLoops { table, loops } => write!(
                f,
                "table {table:?} is anchored on {} loops ({}); a written row opens one loop \
                 instance and nothing in the row says which",
                loops.len(),
                render_names(loops)
            ),
            Refusal::RepeatWithoutInstances {
                table,
                loop_name,
                segment,
            } => write!(
                f,
                "table {table:?} writes one {segment} per row in loop {loop_name:?}, but no \
                 table is anchored on that loop and no column reads it, so the segments have no \
                 instance to go in"
            ),
            Refusal::CodeOutsideList {
                table,
                column,
                loop_name,
                occurrence,
                place,
                value,
                codes,
            } => write!(
                f,
                "{} writes occurrence {occurrence:?} in loop {loop_name:?} with the fixed code \
                 {value:?} in {place}, which its code list ({}) does not allow",
                Source(table, column.as_deref()),
                render_names(codes)
            ),
        }
    }
}

impl Refusal {
    /// What to change in the spec so the refusal goes away, as an imperative
    /// clause.
    pub fn remedy(&self) -> &'static str {
        match self {
            Refusal::RequiredLoopWithoutSource { .. } => {
                "anchor a table on the loop or add a column that reads it, or make the loop \
                 situational in the guide"
            }
            Refusal::RequiredOccurrenceWithoutSource { .. } => {
                "add a column that writes the occurrence, or make it situational in the guide"
            }
            Refusal::RequiredElementWithoutSource { .. } => {
                "add a column that writes the element, give it a single code, or make it \
                 situational in the guide"
            }
            Refusal::AmbiguousSource { column: None, .. } => {
                "anchor the table on a segment that occurs once in the loop"
            }
            Refusal::AmbiguousSource {
                column: Some(_), ..
            } => {
                "name an occurrence the loop has with \"occurrence\", or give the column a \
                 \"where\" that matches exactly one"
            }
            Refusal::PickNotContiguous { .. } => {
                "renumber the picks of the occurrence's columns from 1 with none missing, or \
                 keep a lone \"last\""
            }
            Refusal::WrittenByAnotherTable { .. } => {
                "remove the column, or move it to the table that writes the loop, when a table does"
            }
            Refusal::DuplicateSource { .. } => "remove one of the two columns",
            Refusal::EnvelopeColumn { .. } => {
                "remove the column; the writer takes the envelope's values from the caller's \
                 envelope"
            }
            Refusal::EnvelopeWithoutOccurrence { .. } => {
                "declare the occurrence the loop's trigger opens on"
            }
            Refusal::AnchoredByAnotherTable { .. } => {
                "anchor one of the two tables on a segment of the loop, or remove one of them"
            }
            Refusal::SeveralAnchorLoops { .. } => "anchor the table on one loop",
            Refusal::RepeatWithoutInstances { .. } => {
                "anchor a table on the loop or add a column that reads it"
            }
            Refusal::CodeOutsideList { .. } => {
                "change the fixed code to one the list allows, or add it to the code list in \
                 the guide"
            }
        }
    }
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "spec {:?} cannot be written ({}){}",
            self.spec,
            Counted(self.refusals.len(), "reason", "reasons"),
            Numbered(&self.refusals)
        )
    }
}
