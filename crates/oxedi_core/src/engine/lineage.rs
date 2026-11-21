//! The path of an instance: the occurrence and the ones that enclose it.

use super::{Event, Instance};
use crate::spec::Spec;

/// `instance` and the instances that enclose it, outermost first. `open` is
/// `LoopEngine::path()` after the step and `events` are the events of that step.
///
/// An enclosing instance is either still open or closed by the step's own
/// events (a segment can close loops after it is captured, and a step that
/// opens a loop first closes the ones below its parent). One pass walks the
/// candidates in the order the engine pops them, the closed ones then the open
/// ones from the innermost outwards, which is the order of the parent chain, so
/// each candidate is looked at once. The walk stops where the parent is not
/// among them, keeping what it found.
pub fn lineage(instance: Instance, open: &[Instance], events: &[Event]) -> Vec<Instance> {
    let closed = events.iter().filter_map(|event| match *event {
        Event::LoopClosed { instance } => Some(instance),
        Event::LoopOpened { .. }
        | Event::Captured { .. }
        | Event::Unmatched { .. }
        | Event::Empty { .. } => None,
    });
    let mut candidates = closed.chain(open.iter().rev().copied());
    let mut path = vec![instance];
    let mut wanted = instance.parent;
    while let Some(parent) = wanted {
        let Some(found) = candidates.find(|candidate| candidate.id == parent) else {
            break;
        };
        path.push(found);
        wanted = found.parent;
    }
    path.reverse();
    path
}

/// The loop name and 1-based ordinal of each instance from the outermost down
/// to the one `event` names, empty when the event names none. `open` and
/// `events` are as in [`lineage`].
pub fn named_lineage<'s>(
    spec: &'s Spec,
    event: &Event,
    open: &[Instance],
    events: &[Event],
) -> Vec<(&'s str, usize)> {
    event.instance().map_or_else(Vec::new, |instance| {
        lineage(instance, open, events)
            .iter()
            .map(|at| (spec.loop_name(at.loop_id), at.ordinal_in_stream))
            .collect()
    })
}
