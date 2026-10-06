//! Why a spec cannot be written: the refusals a write plan raises.

use std::fmt;

/// An element or component as X12 names it: `CLP01`, `SVC01-2`.
pub(super) fn place(segment: &[u8], element: usize, component: Option<usize>) -> String {
    let segment = String::from_utf8_lossy(segment);
    match component {
        Some(component) => format!("{segment}{element:02}-{component}"),
        None => format!("{segment}{element:02}"),
    }
}

/// A segment and its conditions as a message shows them.
pub(super) fn render_source(segment: &[u8], conditions: &[(usize, Vec<u8>)]) -> String {
    let segment = String::from_utf8_lossy(segment);
    if conditions.is_empty() {
        return format!("{segment:?}");
    }
    let parts: Vec<String> = conditions
        .iter()
        .map(|(position, value)| format!("{position}: {:?}", String::from_utf8_lossy(value)))
        .collect();
    format!("{segment:?} where {{{}}}", parts.join(", "))
}

/// One reason a spec's tables cannot be turned back into a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// A loop that every instance of its written parent requires has no
    /// table and no column that would give it instances.
    RequiredLoopWithoutSource {
        /// The required loop.
        loop_name: String,
        /// Its parent loop.
        parent: String,
        /// The loop's trigger as the spec writes it.
        trigger: String,
    },
    /// An occurrence that every instance of a written loop requires has no
    /// column that writes it.
    RequiredOccurrenceWithoutSource {
        /// The loop.
        loop_name: String,
        /// The occurrence.
        occurrence: String,
        /// What selects it, e.g. `"PER" where PER01 is "BL"`.
        selector: String,
    },
    /// A required element (or component) of an occurrence that a table
    /// writes has no column, and no single code the spec allows for it.
    RequiredElementWithoutSource {
        /// The table that writes the occurrence; `None` when no table does
        /// (the loop is written only because a loop inside it is).
        table: Option<String>,
        /// The loop.
        loop_name: String,
        /// The occurrence.
        occurrence: String,
        /// The element or component, e.g. `NM102` or `SVC01-1`.
        place: String,
        /// The element's name in the spec.
        name: String,
    },
    /// A column's segment and `where` (or a table's anchor segment) match no
    /// occurrence of the loop, or more than one, so the segment it writes
    /// cannot be placed.
    AmbiguousSource {
        /// The table.
        table: String,
        /// The column; `None` for the table's anchor segment.
        column: Option<String>,
        /// The loop the column reads.
        loop_name: String,
        /// The segment and conditions, e.g. `"DTM" where {1: "232"}`.
        source: String,
        /// The occurrences that match; empty when none does.
        candidates: Vec<String>,
    },
    /// The columns that read one occurrence pick its repeats out of order:
    /// the picks must be `first` or 1 up to some n with none missing, or a
    /// lone `last`.
    PickNotContiguous {
        /// The table.
        table: String,
        /// The first column whose pick breaks the series.
        column: String,
        /// The loop.
        loop_name: String,
        /// The occurrence.
        occurrence: String,
        /// Every pick the occurrence's columns use, as the spec writes them.
        picks: Vec<String>,
    },
    /// A column reads a loop whose instances another table (or the
    /// envelope) already writes, so its value has no place of its own.
    WrittenByAnotherTable {
        /// The table.
        table: String,
        /// The column.
        column: String,
        /// The loop the column reads.
        loop_name: String,
        /// What writes the loop: a table's name, or `the envelope`.
        writer: String,
    },
    /// Two columns write the same element of the same segment.
    DuplicateSource {
        /// The table.
        table: String,
        /// The second column.
        column: String,
        /// The column that already writes the element.
        other: String,
        /// The loop.
        loop_name: String,
        /// The occurrence.
        occurrence: String,
        /// The element or component, e.g. `CLP01`.
        place: String,
    },
    /// A column reads an envelope segment, whose values the caller's
    /// envelope and the writer's counts give.
    EnvelopeColumn {
        /// The table.
        table: String,
        /// The column.
        column: String,
        /// The envelope loop.
        loop_name: String,
        /// The envelope occurrence.
        occurrence: String,
    },
    /// A table without a segment is anchored on a loop whose rows another
    /// table already gives.
    AnchoredByAnotherTable {
        /// The table.
        table: String,
        /// The loop it is anchored on.
        loop_name: String,
        /// The table that already gives the loop its rows.
        other: String,
    },
    /// A table anchored on a segment sits in a loop that nothing gives
    /// instances to, so its segments have no instance to go in.
    RepeatWithoutInstances {
        /// The table.
        table: String,
        /// The loop it is anchored on.
        loop_name: String,
        /// The anchor segment, e.g. `"CAS"`.
        segment: String,
    },
    /// A code the spec fixes for a written segment (a `where` value or the
    /// occurrence's single qualifier code) is outside the code list of the
    /// occurrence or of the element.
    CodeOutsideList {
        /// The table.
        table: String,
        /// The column whose segment carries the code; `None` for the
        /// table's anchor segment.
        column: Option<String>,
        /// The loop.
        loop_name: String,
        /// The occurrence.
        occurrence: String,
        /// The element or component, e.g. `PER01`.
        place: String,
        /// The fixed code.
        value: String,
        /// The code list that does not allow it.
        codes: Vec<String>,
    },
}

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
            } => {
                write!(
                    f,
                    "required element {place} ({name}) of occurrence {occurrence:?} in loop {loop_name:?}"
                )?;
                match table {
                    Some(table) => write!(f, ", written by table {table:?},")?,
                    None => write!(f, ", which no table writes,")?,
                }
                write!(f, " has no column and no single code to write")
            }
            Refusal::AmbiguousSource {
                table,
                column,
                loop_name,
                source,
                candidates,
            } => {
                match column {
                    Some(column) => write!(f, "table {table:?} column {column:?}")?,
                    None => write!(f, "table {table:?} anchor segment")?,
                }
                write!(f, " reads {source} in loop {loop_name:?}, which ")?;
                if candidates.is_empty() {
                    write!(f, "matches none of its occurrences")
                } else {
                    let quoted: Vec<String> =
                        candidates.iter().map(|name| format!("{name:?}")).collect();
                    write!(
                        f,
                        "matches {} of its occurrences ({}); name one with \"occurrence\"",
                        candidates.len(),
                        quoted.join(", ")
                    )
                }
            }
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
            Refusal::AnchoredByAnotherTable {
                table,
                loop_name,
                other,
            } => write!(
                f,
                "table {table:?} is anchored on loop {loop_name:?}, whose rows table {other:?} \
                 already gives; a loop takes its rows from one table"
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
            } => {
                match column {
                    Some(column) => write!(f, "table {table:?} column {column:?}")?,
                    None => write!(f, "table {table:?} anchor segment")?,
                }
                let quoted: Vec<String> = codes.iter().map(|code| format!("{code:?}")).collect();
                write!(
                    f,
                    " writes occurrence {occurrence:?} in loop {loop_name:?} with the fixed code \
                     {value:?} in {place}, which its code list ({}) does not allow",
                    quoted.join(", ")
                )
            }
        }
    }
}

/// A spec whose tables cannot be written, with every refusal found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanError {
    /// The spec's name.
    pub spec: String,
    /// Every reason, in the order the plan found them.
    pub refusals: Vec<Refusal>,
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.refusals.len();
        let noun = if count == 1 { "reason" } else { "reasons" };
        write!(f, "spec {:?} cannot be written ({count} {noun})", self.spec)?;
        for (i, refusal) in self.refusals.iter().enumerate() {
            write!(f, "\n{}. {refusal}", i + 1)?;
        }
        Ok(())
    }
}

impl std::error::Error for PlanError {}
