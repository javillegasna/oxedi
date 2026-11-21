//! The loops that enclose chosen segments of a stream.

use std::collections::{HashMap, HashSet};

use super::{Event, Instance, LoopEngine, lineage};
use crate::segment::Segment;
use crate::spec::Spec;

/// For each index of `wanted`, in the order given, the loop instances that
/// enclose that segment, outermost first. Segment indexes are 0-based.
///
/// A segment that closes its loop keeps that loop in its path, so an end
/// segment still lists the loop it ends. An unmatched or empty segment has the
/// loops open around it. An index past the last segment has an empty path, and
/// a repeated index gets the same path each time.
///
/// One pass over `segments` that stops after the highest wanted index; only
/// the wanted segments copy their step's events.
pub fn loop_paths<'a>(
    spec: &Spec,
    segments: impl IntoIterator<Item = Segment<'a>>,
    wanted: &[usize],
) -> Vec<Vec<Instance>> {
    let targets: HashSet<usize> = wanted.iter().copied().collect();
    let Some(last) = targets.iter().copied().max() else {
        return Vec::new();
    };
    let mut engine = LoopEngine::new(spec);
    let mut found: HashMap<usize, Vec<Instance>> = HashMap::with_capacity(targets.len());
    for segment in segments.into_iter().take(last.saturating_add(1)) {
        let events = engine.feed(&segment);
        if !targets.contains(&segment.index) {
            continue;
        }
        let events = events.to_vec();
        found.insert(segment.index, path_of_step(&engine, &events));
    }
    wanted
        .iter()
        .map(|index| found.get(index).cloned().unwrap_or_default())
        .collect()
}

/// The path of the segment `events` came from: the lineage of its capture
/// (which keeps the loops the segment closed), or the engine's open path when
/// nothing captured it.
fn path_of_step(engine: &LoopEngine<'_>, events: &[Event]) -> Vec<Instance> {
    let captured = events.iter().find_map(|event| match *event {
        Event::Captured { instance, .. } => Some(instance),
        _ => None,
    });
    match captured {
        Some(instance) => lineage(instance, engine.path(), events),
        None => engine.path().to_vec(),
    }
}
