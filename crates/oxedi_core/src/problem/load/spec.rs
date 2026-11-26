//! The text of a spec that did not load: the error that wraps every other.

use std::fmt;

use super::text::{Under, write_ambiguous, write_cycle};
use crate::spec_load::SpecError;

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
            SpecError::BadKit { reason } => write!(f, "the kit is invalid: {reason}"),
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
