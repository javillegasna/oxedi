//! The path of an instance: the occurrence and the ones that enclose it.

use super::{Event, Instance};

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
