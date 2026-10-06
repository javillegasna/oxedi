//! The error returned when a spec fails to load.

use std::fmt;

use super::loops::ControlError;
use super::occurrences::OccurrenceError;
use super::segments::ElementDefError;
use super::tables::TableDefError;
use super::version::VersionError;

/// Why a spec could not be loaded. Each variant names where in the spec the fault is.
#[derive(Debug)]
pub enum SpecError {
    /// The text is not valid JSON.
    Json(serde_json::Error),
    /// The JSON is valid but does not match the spec schema.
    Schema {
        /// The loop whose definition is malformed; `None` when the top level is.
        loop_name: Option<String>,
        /// What serde rejected.
        source: serde_json::Error,
    },
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
    /// A `where` key is not a 1-based element position in canonical form.
    BadPosition {
        /// The loop with the bad key.
        loop_name: String,
        /// The key as written.
        position: String,
    },
    /// A value the schema requires to be an object is something else.
    NotAnObject {
        /// Where the value sits, keys joined by `.` as written (e.g.
        /// `loops.2100.trigger`); empty for the top level.
        path: String,
        /// What was found instead: `an array`, `a string`, `a number`,
        /// `a boolean` or `null`.
        found: &'static str,
    },
    /// A scalar the schema requires to be of one kind is of another.
    WrongType {
        /// Where the value sits, keys joined by `.` as written (e.g.
        /// `loops.env.control.opener_element`).
        path: String,
        /// What the schema requires, e.g. `a non-negative integer`.
        expected: &'static str,
        /// What was found instead: `a string`, `a number`, `a negative
        /// number`, `an array`, `null`, and so on.
        found: &'static str,
        /// The offending value: a scalar as compact JSON (`"2"`, `7`,
        /// `true`), an array or object as its length (`3 items`,
        /// `2 members`); empty for `null`.
        value: String,
    },
    /// An object holds a key its schema does not define.
    UnknownKey {
        /// Where the object sits, keys joined by `.` as written (e.g.
        /// `segments.CLP.elements.3`); empty for the top level.
        path: String,
        /// The key as written.
        key: String,
    },
    /// An object lacks a key its schema requires.
    MissingKey {
        /// Where the object sits, keys joined by `.` as written (e.g.
        /// `loops.env.control`); empty for the top level.
        path: String,
        /// The required key.
        key: &'static str,
    },
    /// A segment definition does not match the schema.
    SegmentSchema {
        /// The segment id as written.
        segment: String,
        /// What serde rejected.
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
    /// A table or column definition does not match the schema.
    TableSchema {
        /// The table name as written.
        table: String,
        /// The column name as written, when the fault is inside a column.
        column: Option<String>,
        /// What serde rejected.
        source: serde_json::Error,
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
    /// A loop declares occurrences, and none of them is the one its trigger
    /// opens on.
    UnmatchedTrigger {
        /// The loop.
        loop_name: String,
        /// The trigger, e.g. `"N1" where {1: "PR"}`.
        trigger: String,
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
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpecError::Json(e) => write!(f, "invalid spec JSON: {e}"),
            SpecError::Schema {
                loop_name: Some(loop_name),
                source,
            } => write!(f, "loop {loop_name:?} does not match the schema: {source}"),
            SpecError::Schema {
                loop_name: None,
                source,
            } => write!(f, "spec does not match the schema: {source}"),
            SpecError::NotAnObject { path, found } if path.is_empty() => {
                write!(f, "the spec must be a JSON object; found {found}")
            }
            SpecError::NotAnObject { path, found } => write!(
                f,
                "spec: the value at {path} must be a JSON object; found {found}"
            ),
            SpecError::WrongType {
                path,
                expected,
                found,
                value,
            } if value.is_empty() => write!(
                f,
                "spec: the value at {path} must be {expected}; found {found}"
            ),
            SpecError::WrongType {
                path,
                expected,
                found,
                value,
            } => write!(
                f,
                "spec: the value at {path} must be {expected}; found {found} ({value})"
            ),
            SpecError::UnknownKey { path, key } if path.is_empty() => {
                write!(f, "spec: unknown key {key:?} at the top level")
            }
            SpecError::UnknownKey { path, key } => {
                write!(f, "spec: unknown key {key:?} at {path}")
            }
            SpecError::MissingKey { path, key } if path.is_empty() => {
                write!(f, "spec: missing required key {key:?} at the top level")
            }
            SpecError::MissingKey { path, key } => {
                write!(f, "spec: missing required key {key:?} at {path}")
            }
            SpecError::Patch { source } => write!(f, "applying patch: {source}"),
            SpecError::UnknownParent { loop_name, parent } => {
                write!(f, "loop {loop_name:?} names unknown parent {parent:?}")
            }
            SpecError::Cycle { members } => {
                write!(f, "loops form a parent cycle: ")?;
                for member in members {
                    write!(f, "{member} -> ")?;
                }
                match members.first() {
                    Some(first) => write!(f, "{first}"),
                    None => Ok(()),
                }
            }
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
            SpecError::BadPosition {
                loop_name,
                position,
            } => write!(
                f,
                "loop {loop_name:?} has an invalid \"where\" position {position:?}: \
                 positions are 1-based integers written in canonical form"
            ),
            SpecError::SegmentSchema { segment, source } => {
                write!(f, "segment {segment:?} does not match the schema: {source}")
            }
            SpecError::BadElementDef {
                segment,
                position,
                reason,
            } => write!(f, "segment {segment:?} element {position:?}: {reason}"),
            SpecError::TableSchema {
                table,
                column: None,
                source,
            } => write!(f, "table {table:?} does not match the schema: {source}"),
            SpecError::TableSchema {
                table,
                column: Some(column),
                source,
            } => write!(
                f,
                "table {table:?} column {column:?} does not match the schema: {source}"
            ),
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
            SpecError::UnmatchedTrigger { loop_name, trigger } => write!(
                f,
                "loop {loop_name:?} opens on {trigger}, but none of its occurrences holds that \
                 segment with a qualifier the trigger's conditions select"
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
            } => {
                write!(f, "loops {first:?} and {second:?} under ")?;
                match parent {
                    Some(parent) => write!(f, "{parent:?}")?,
                    None => write!(f, "the root")?,
                }
                write!(f, " share the identical trigger {segment:?}")?;
                if !conditions.is_empty() {
                    write!(f, " where {{")?;
                    for (i, (position, value)) in conditions.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{position}: {value:?}")?;
                    }
                    write!(f, "}}")?;
                }
                Ok(())
            }
            SpecError::OverlappingTriggers {
                parent,
                a,
                b,
                conditions_a,
                conditions_b,
            } => {
                write!(f, "loops {a:?} and {b:?} under ")?;
                match parent {
                    Some(parent) => write!(f, "{parent:?}")?,
                    None => write!(f, "the root")?,
                }
                write!(
                    f,
                    " can open on the same segment: {a:?} on {conditions_a}, {b:?} on \
                     {conditions_b}, no position they both test requires different values, \
                     and neither trigger is more specific than the other"
                )
            }
        }
    }
}

impl std::error::Error for SpecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SpecError::Json(e)
            | SpecError::Schema { source: e, .. }
            | SpecError::SegmentSchema { source: e, .. }
            | SpecError::TableSchema { source: e, .. } => Some(e),
            SpecError::Patch { source } => Some(source.as_ref()),
            _ => None,
        }
    }
}
