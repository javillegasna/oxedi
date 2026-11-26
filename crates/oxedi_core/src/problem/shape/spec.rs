//! The shape of a spec error.

use super::{Place, Problem, SpecPlace};
use crate::spec::BatchLoopError;
use crate::spec_load::SpecError;

problem!(SpecError [Json, Patch, UnknownParent, Cycle, NoLoops, EmptySegmentId, EndIsTrigger, Format, BadElementDef, BadTable, BadHeader, BadKit, Package, Stack, BadVersion, BadControl, BadOccurrence, BadBalance, UnmatchedTrigger, TriggerQualifierWider, TriggerNotFirst, ZeroLoopMax, AmbiguousTrigger, OverlappingTriggers, BadSummary, UnknownDefaultTable] this => {
        let in_loop = |name, occurrence| Place::Spec(SpecPlace::Loop { name, occurrence });
        let table = |name, column| Place::Spec(SpecPlace::Table { name, column });
        match this {
            SpecError::Json(_)
            | SpecError::NoLoops { .. }
            | SpecError::BadHeader { .. }
            | SpecError::Stack { .. }
            | SpecError::BadVersion { .. } => Place::Spec(SpecPlace::Whole),
            SpecError::Patch { source } => source.place(),
            SpecError::BadKit { reason } => reason.place(),
            SpecError::Package { id, .. } => Place::Package { id },
            SpecError::Format { path, .. } => Place::Spec(SpecPlace::Path(path)),
            SpecError::UnknownParent { loop_name, .. }
            | SpecError::EndIsTrigger { loop_name, .. }
            | SpecError::BadControl { loop_name, .. }
            | SpecError::UnmatchedTrigger { loop_name, .. }
            | SpecError::ZeroLoopMax { loop_name } => in_loop(loop_name, None),
            SpecError::BadOccurrence {
                loop_name,
                occurrence,
                ..
            }
            | SpecError::TriggerQualifierWider {
                loop_name,
                occurrence,
                ..
            }
            | SpecError::TriggerNotFirst {
                loop_name,
                occurrence,
                ..
            } => in_loop(loop_name, Some(occurrence)),
            SpecError::AmbiguousTrigger { first: name, .. }
            | SpecError::OverlappingTriggers { a: name, .. } => in_loop(name, None),
            SpecError::Cycle { members } => members
                .first()
                .map_or(Place::Spec(SpecPlace::Whole), |name| in_loop(name, None)),
            SpecError::EmptySegmentId { key, .. } => Place::Spec(SpecPlace::Path(key)),
            SpecError::BadElementDef {
                segment, position, ..
            } => Place::Spec(SpecPlace::Segment {
                id: segment,
                position: Some(position),
            }),
            SpecError::BadTable {
                table: name,
                column,
                ..
            } => table(name, column.as_deref()),
            SpecError::UnknownDefaultTable { table: name, .. } => table(name, None),
            SpecError::BadBalance { rule, .. } => Place::Spec(SpecPlace::Balance { rule }),
            SpecError::BadSummary { summary, .. } => {
                Place::Spec(SpecPlace::Summary { name: summary })
            }
        }
});

problem!(BatchLoopError [Unknown, NoEnvelope] _this => Place::Spec(SpecPlace::Whole));
