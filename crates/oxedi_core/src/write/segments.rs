//! The segments a loop's columns write: one per repeat their picks name,
//! with the codes that select it and the column of each element.

use std::collections::BTreeMap;

use crate::spec::OccurrenceDef;
use crate::vocabulary::Pick;

use super::build::{Builder, Codes, Entry};
use super::plan::{ElementPlan, Place, SegmentPlan, SegmentSource, ValueSource};
use super::refusal::Refusal;
use super::required::add_qualifier;
use crate::message::place as place_name;

impl Builder<'_> {
    /// Turns the grouped column values into segments: one per repeat the
    /// columns pick, with the codes that select it.
    pub(super) fn segments(&mut self) {
        for ((id, at, _, codes), entries) in std::mem::take(&mut self.groups) {
            self.group_segments(id, at, &codes, &entries);
        }
    }

    /// The segments of one occurrence and set of `where` codes, added to
    /// its loop's plan.
    fn group_segments(&mut self, id: usize, at: usize, codes: &Codes, entries: &[Entry]) {
        let Some(occurrence) = self.occurrence(id, at).cloned() else {
            return;
        };
        let Some(nths) = self.nths(id, &occurrence, entries) else {
            return;
        };
        let mut by_nth: BTreeMap<usize, SegmentPlan> = BTreeMap::new();
        for (entry, nth) in entries.iter().zip(nths) {
            let plan = by_nth
                .entry(nth)
                .or_insert_with(|| new_segment(at, nth, entry.table, codes, &occurrence));
            self.place_column(id, &occurrence, plan, entry);
        }
        if let Some(carrier) = entries.first() {
            for plan in by_nth.values() {
                self.check_codes(id, plan, carrier.table, Some(carrier.column));
            }
        }
        if let Some(plan) = self.loops.get_mut(id) {
            plan.segments.extend(by_nth.into_values());
        }
    }

    /// Puts one column in a segment's plan. A column replaces the codes the
    /// spec fixes on the places it overlaps, and is refused when another
    /// column already writes one of them.
    fn place_column(
        &mut self,
        id: usize,
        occurrence: &OccurrenceDef,
        plan: &mut SegmentPlan,
        entry: &Entry,
    ) {
        let place = Place {
            element: entry.element,
            component: entry.component,
        };
        let written = plan.elements.iter().find_map(|element| {
            match (&element.value, element.place().overlaps(place)) {
                (ValueSource::Column { column }, true) => Some(*column),
                _ => None,
            }
        });
        if let Some(other) = written {
            self.refusals.push(Refusal::DuplicateSource {
                table: self.table_name(entry.table),
                column: self.column_name(entry.table, entry.column),
                other: self.column_name(entry.table, other),
                loop_name: self.loop_name(id),
                occurrence: occurrence.name.clone(),
                place: place_name(&occurrence.segment, entry.element, entry.component),
            });
            return;
        }
        plan.elements
            .retain(|element| !element.place().overlaps(place));
        plan.elements.push(ElementPlan {
            element: entry.element,
            component: entry.component,
            value: ValueSource::Column {
                column: entry.column,
            },
        });
    }

    /// The repeat (from 1) each entry writes, or `None` after refusing a
    /// series of picks with a gap or a `last` among other picks.
    fn nths(
        &mut self,
        id: usize,
        occurrence: &OccurrenceDef,
        entries: &[Entry],
    ) -> Option<Vec<usize>> {
        let nths = entry_nths(entries);
        let Some(entry) = first_broken(&nths).and_then(|at| entries.get(at)) else {
            return Some(nths.into_iter().flatten().collect());
        };
        let mut picks: Vec<String> = entries.iter().map(|entry| entry.pick.to_string()).collect();
        picks.sort();
        picks.dedup();
        self.refusals.push(Refusal::PickNotContiguous {
            table: self.table_name(entry.table),
            column: self.column_name(entry.table, entry.column),
            loop_name: self.loop_name(id),
            occurrence: occurrence.name.clone(),
            picks,
        });
        None
    }
}

/// The repeat each entry's pick names, or `None` for a `last` that shares
/// the occurrence with other picks.
fn entry_nths(entries: &[Entry]) -> Vec<Option<usize>> {
    let lone_last = entries.iter().all(|entry| entry.pick == Pick::Last);
    entries
        .iter()
        .map(|entry| match entry.pick {
            Pick::First => Some(1),
            Pick::Nth(nth) => Some(nth),
            Pick::Last if lone_last => Some(1),
            Pick::Last => None,
        })
        .collect()
}

/// The index of the first entry that breaks the series: a `last` among
/// other picks, else the first entry of the lowest repeat that leaves a
/// gap below it.
fn first_broken(nths: &[Option<usize>]) -> Option<usize> {
    if let Some(last) = nths.iter().position(Option::is_none) {
        return Some(last);
    }
    let mut distinct: Vec<usize> = nths.iter().flatten().copied().collect();
    distinct.sort_unstable();
    distinct.dedup();
    let gap = distinct
        .iter()
        .enumerate()
        .find_map(|(i, nth)| (*nth != i + 1).then_some(*nth))?;
    nths.iter().position(|nth| *nth == Some(gap))
}

/// The plan of the `nth` segment of `occurrence` filled from a row of
/// `table`: the `where` codes and the qualifier code come first.
fn new_segment(
    at: usize,
    nth: usize,
    table: usize,
    codes: &Codes,
    occurrence: &OccurrenceDef,
) -> SegmentPlan {
    let mut plan = SegmentPlan {
        occurrence: at,
        source: SegmentSource::Row { table, nth },
        elements: codes
            .iter()
            .map(|(position, value)| ElementPlan {
                element: *position,
                component: None,
                value: ValueSource::Code(value.clone()),
            })
            .collect(),
    };
    add_qualifier(&mut plan, occurrence);
    plan
}
