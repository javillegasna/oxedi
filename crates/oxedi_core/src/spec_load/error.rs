//! The error returned when a spec fails to load.

use std::fmt;

use super::balance_error::BalanceError;
use super::build::VersionError;
use super::build_loops::ControlError;
use super::error_text::{Under, write_ambiguous, write_cycle};
use super::occurrence_error::OccurrenceError;
use super::package::{HeaderError, PackageId, StackError};
use super::segments::ElementDefError;
use super::summary_error::SummaryDefError;
use super::table_error::TableDefError;

/// Why a spec could not be loaded. Each variant names where in the spec the fault is.
#[derive(Debug)]
pub enum SpecError {
    /// The text is not valid JSON.
    Json(serde_json::Error),
    /// Applying a patch produced an error.
    Patch {
        /// The error the patched spec (or the patch text) produced.
        source: Box<SpecError>,
    },
    /// A loop names a parent that does not exist.
    UnknownParent {
        /// The loop with the bad reference.
        loop_name: String,
        /// The missing parent.
        parent: String,
    },
    /// Following `parent` links never reaches a top-level loop.
    Cycle {
        /// The loops on the cycle in walk order, starting at the one that repeats.
        members: Vec<String>,
    },
    /// The `loops` map is empty.
    NoLoops {
        /// The spec's `name`.
        spec_name: String,
    },
    /// A segment id is the empty string.
    EmptySegmentId {
        /// The loop holding it; `None` for a key of the `segments` section.
        loop_name: Option<String>,
        /// Where it sits, as written: `trigger.segment`,
        /// `occurrences.<name>.segment`, `end`, or `segments.""` for the section.
        key: String,
    },
    /// A loop's `end` is the segment that opens it.
    EndIsTrigger {
        /// The loop.
        loop_name: String,
        /// The segment id both keys name.
        segment: String,
    },
    /// A value the spec's model does not read, anywhere in the spec (the first
    /// in key order is reported): a key it does not take, a missing key, a value
    /// of the wrong JSON type or `null` where a value belongs, a word outside its
    /// vocabulary (a usage, a sign), an element type, a key that is not a
    /// position, a summary that holds no operation or more than one.
    Format {
        /// Where the value sits, from the root: keys joined by `.`, a key holding
        /// `.`, `/`, `#` or whitespace quoted (e.g. `loops.2100.occurrences.clp.usage`);
        /// `.` for the root.
        path: String,
        /// What the model refused, with the offending value.
        source: serde_json::Error,
    },
    /// An element definition is invalid.
    BadElementDef {
        /// The segment id as written.
        segment: String,
        /// The element's key as written; a component is `<element>.composite.<component>`.
        position: String,
        /// What is wrong with it.
        reason: ElementDefError,
    },
    /// A table definition is invalid.
    BadTable {
        /// The table name as written.
        table: String,
        /// The column name as written, when the fault is inside a column.
        column: Option<String>,
        /// What is wrong with it.
        reason: TableDefError,
    },
    /// A package header is invalid.
    BadHeader {
        /// What is wrong with it.
        reason: HeaderError,
    },
    /// The cumulative value after a package does not read as the model.
    Package {
        /// The package whose spec made the value unreadable.
        id: PackageId,
        /// Why the value does not read.
        source: Box<SpecError>,
    },
    /// A stack of packages is invalid.
    Stack {
        /// What is wrong with it.
        reason: Box<StackError>,
    },
    /// The spec's `version` is invalid.
    BadVersion {
        /// What is wrong with it.
        reason: VersionError,
    },
    /// A loop's `control` is invalid.
    BadControl {
        /// The loop.
        loop_name: String,
        /// What is wrong with it.
        reason: ControlError,
    },
    /// An occurrence of a loop is invalid.
    BadOccurrence {
        /// The loop.
        loop_name: String,
        /// The occurrence name as written.
        occurrence: String,
        /// What is wrong with it.
        reason: Box<OccurrenceError>,
    },
    /// A balancing rule is invalid.
    BadBalance {
        /// The rule name as written.
        rule: String,
        /// What is wrong with it.
        reason: Box<BalanceError>,
    },
    /// A loop declares occurrences, and none of them is the one its trigger
    /// opens on.
    UnmatchedTrigger {
        /// The loop.
        loop_name: String,
        /// The trigger, e.g. `"N1" where {1: "PR"}`.
        trigger: String,
        /// Whether some occurrence holds the trigger's segment, with a
        /// qualifier the trigger's conditions do not select.
        segment_held: bool,
    },
    /// The qualifier of the occurrence a loop's trigger opens on accepts a
    /// code the trigger's conditions do not select: a segment with that code
    /// would match the occurrence without opening the loop.
    TriggerQualifierWider {
        /// The loop.
        loop_name: String,
        /// The trigger's occurrence.
        occurrence: String,
        /// The trigger, e.g. `"N1" where {1: "PR"}`.
        trigger: String,
        /// The first code the trigger does not select.
        code: String,
    },
    /// Another occurrence of a loop has a position at or before the
    /// occurrence its trigger opens on, which must come first.
    TriggerNotFirst {
        /// The loop.
        loop_name: String,
        /// The trigger's occurrence.
        occurrence: String,
        /// Its position.
        pos: usize,
        /// The occurrence placed at or before it.
        other: String,
        /// That occurrence's position.
        other_pos: usize,
    },
    /// A loop's `max` is 0.
    ZeroLoopMax {
        /// The loop.
        loop_name: String,
    },
    /// Two loops with the same parent have identical triggers.
    AmbiguousTrigger {
        /// First loop, in spec order.
        first: String,
        /// Second loop, in spec order.
        second: String,
        /// Their common parent; `None` for top-level loops.
        parent: Option<String>,
        /// The shared trigger segment id.
        segment: String,
        /// The shared trigger conditions, sorted by position.
        conditions: Vec<(usize, String)>,
    },
    /// Two loops with the same parent trigger on the same segment id, no
    /// position tested by both requires different values, and neither set of
    /// conditions contains the other: one segment can satisfy both and
    /// neither trigger is more specific.
    OverlappingTriggers {
        /// Their common parent; `None` for top-level loops.
        parent: Option<String>,
        /// First loop, in spec order.
        a: String,
        /// Second loop, in spec order.
        b: String,
        /// The first loop's trigger, e.g. `"N1" where {1: "PR"}`.
        conditions_a: String,
        /// The second loop's trigger, written the same way.
        conditions_b: String,
    },
    /// A summary is invalid.
    BadSummary {
        /// The summary name as written.
        summary: String,
        /// What is wrong with it.
        reason: Box<SummaryDefError>,
    },
    /// `default_table` names a table the spec does not define.
    UnknownDefaultTable {
        /// The table as written.
        table: String,
        /// The tables the spec defines, in name order.
        known: Vec<String>,
    },
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpecError::Json(e) => write!(f, "invalid spec JSON: {e}"),
            SpecError::Patch { source } => write!(f, "applying patch: {source}"),
            SpecError::UnknownParent { loop_name, parent } => {
                write!(f, "loop {loop_name:?} names unknown parent {parent:?}")
            }
            SpecError::Cycle { members } => write_cycle(f, members),
            SpecError::NoLoops { spec_name } => {
                write!(f, "spec {spec_name:?} declares no loops")
            }
            SpecError::EmptySegmentId {
                loop_name: Some(loop_name),
                key,
            } => write!(f, "loop {loop_name:?} has an empty segment id at {key}"),
            SpecError::EmptySegmentId {
                loop_name: None,
                key,
            } => write!(f, "the spec has an empty segment id at {key}"),
            SpecError::EndIsTrigger { loop_name, segment } => write!(
                f,
                "loop {loop_name:?} has \"end\" {segment:?}, the same segment as its \
                 \"trigger\": the loop would close on the segment that opens it"
            ),
            SpecError::Format { path, source } => write!(f, "spec: at {path}: {source}"),
            SpecError::BadElementDef {
                segment,
                position,
                reason,
            } => write!(f, "segment {segment:?} element {position:?}: {reason}"),
            SpecError::BadTable {
                table,
                column: None,
                reason,
            } => write!(f, "table {table:?}: {reason}"),
            SpecError::BadTable {
                table,
                column: Some(column),
                reason,
            } => write!(f, "table {table:?} column {column:?}: {reason}"),
            SpecError::BadHeader { reason } => {
                write!(f, "the package header is invalid: {reason}")
            }
            SpecError::Package { id, source } => write!(f, "package \"{id}\": {source}"),
            SpecError::Stack { reason } => write!(f, "{reason}"),
            SpecError::BadVersion { reason } => {
                write!(f, "the spec's \"version\" is invalid: {reason}")
            }
            SpecError::BadControl { loop_name, reason } => {
                write!(f, "loop {loop_name:?} has an invalid \"control\": {reason}")
            }
            SpecError::BadOccurrence {
                loop_name,
                occurrence,
                reason,
            } => write!(f, "loop {loop_name:?} occurrence {occurrence:?}: {reason}"),
            SpecError::BadBalance { rule, reason } => {
                write!(f, "balancing rule {rule:?}: {reason}")
            }
            SpecError::UnmatchedTrigger {
                loop_name,
                trigger,
                segment_held: true,
            } => write!(
                f,
                "loop {loop_name:?} opens on {trigger}, but none of its occurrences holds that \
                 segment with a qualifier the trigger's conditions select"
            ),
            SpecError::UnmatchedTrigger {
                loop_name,
                trigger,
                segment_held: false,
            } => write!(
                f,
                "loop {loop_name:?} opens on {trigger}, but none of its occurrences holds that \
                 segment: declare the occurrence the loop opens on"
            ),
            SpecError::TriggerQualifierWider {
                loop_name,
                occurrence,
                trigger,
                code,
            } => write!(
                f,
                "loop {loop_name:?} opens on {trigger}, and its occurrence {occurrence:?} also \
                 accepts code {code:?}: a segment with that code would match the occurrence \
                 without opening the loop"
            ),
            SpecError::TriggerNotFirst {
                loop_name,
                occurrence,
                pos,
                other,
                other_pos,
            } => write!(
                f,
                "loop {loop_name:?}: occurrence {other:?} has \"pos\" {other_pos}, at or before \
                 {pos} of {occurrence:?}, the occurrence the loop opens on, which must come first"
            ),
            SpecError::ZeroLoopMax { loop_name } => write!(
                f,
                "loop {loop_name:?} has \"max\" 0; a loop that may appear appears at least once"
            ),
            SpecError::AmbiguousTrigger {
                first,
                second,
                parent,
                segment,
                conditions,
            } => write_ambiguous(f, [first, second], parent.as_deref(), segment, conditions),
            SpecError::OverlappingTriggers {
                parent,
                a,
                b,
                conditions_a,
                conditions_b,
            } => write!(
                f,
                "loops {a:?} and {b:?} under {} can open on the same segment: {a:?} on \
                 {conditions_a}, {b:?} on {conditions_b}, no position they both test requires \
                 different values, and neither trigger is more specific than the other",
                Under(parent.as_deref())
            ),
            SpecError::BadSummary { summary, reason } => write!(f, "summary {summary:?}: {reason}"),
            SpecError::UnknownDefaultTable { table, known } => write!(
                f,
                "default_table names the table {table:?}, which the spec does not define; \
                 its tables are: {}",
                known.join(", ")
            ),
        }
    }
}
