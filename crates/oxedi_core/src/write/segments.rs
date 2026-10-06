//! The segments a loop's columns write: one per repeat their picks name,
//! with the codes that select it and the column of each element.

use std::collections::BTreeMap;

use crate::spec::{OccurrenceDef, Pick};

use super::build::{Builder, Entry};
use super::plan::{ElementPlan, SegmentPlan, SegmentSource, ValueSource};
use super::refusal::{Refusal, place};
use super::required::add_qualifier;

impl Builder<'_> {
    /// Turns the grouped column values into segments: one per repeat the
    /// columns pick, with the codes that select it.
    pub(super) fn segments(&mut self) {
        let groups = std::mem::take(&mut self.groups);
        for ((id, at, _, codes), entries) in groups {
            let Some(occurrence) = self.occurrence(id, at).cloned() else {
                continue;
            };
            let Some(nths) = self.nths(id, &occurrence, &entries) else {
                continue;
            };
            let mut by_nth: BTreeMap<usize, SegmentPlan> = BTreeMap::new();
            for (entry, nth) in entries.iter().zip(nths) {
                let plan = by_nth.entry(nth).or_insert_with(|| {
                    let mut plan = SegmentPlan {
                        occurrence: at,
                        source: SegmentSource::Row {
                            table: entry.table,
                            nth,
                        },
                        elements: codes
                            .iter()
                            .map(|(position, value)| ElementPlan {
                                element: *position,
                                component: None,
                                value: ValueSource::Code(value.clone()),
                            })
                            .collect(),
                    };
                    add_qualifier(&mut plan, &occurrence);
                    plan
                });
                let existing = plan.elements.iter_mut().find(|element| {
                    (element.element, element.component) == (entry.element, entry.component)
                });
                match existing {
                    Some(ElementPlan {
                        value: ValueSource::Column { column },
                        ..
                    }) => {
                        let other = self.column_name(entry.table, *column);
                        self.refusals.push(Refusal::DuplicateSource {
                            table: self.table_name(entry.table),
                            column: self.column_name(entry.table, entry.column),
                            other,
                            loop_name: self
                                .spec
                                .loops()
                                .get(id)
                                .map(|def| def.name.clone())
                                .unwrap_or_default(),
                            occurrence: occurrence.name.clone(),
                            place: place(&occurrence.segment, entry.element, entry.component),
                        });
                    }
                    Some(element) => {
                        element.value = ValueSource::Column {
                            column: entry.column,
                        };
                    }
                    None => plan.elements.push(ElementPlan {
                        element: entry.element,
                        component: entry.component,
                        value: ValueSource::Column {
                            column: entry.column,
                        },
                    }),
                }
            }
            let carrier = entries.first().map(|entry| (entry.table, entry.column));
            for plan in by_nth.values() {
                if let Some((table, column)) = carrier {
                    self.check_codes(id, plan, table, Some(column));
                }
            }
            if let Some(plan) = self.loops.get_mut(id) {
                plan.segments.extend(by_nth.into_values());
            }
        }
    }

    /// The repeat (from 1) each entry writes, or `None` after refusing a
    /// series of picks with a gap or a `last` among other picks.
    fn nths(
        &mut self,
        id: usize,
        occurrence: &OccurrenceDef,
        entries: &[Entry],
    ) -> Option<Vec<usize>> {
        let lone_last = entries.iter().all(|entry| entry.pick == Pick::Last);
        let nths: Vec<Option<usize>> = entries
            .iter()
            .map(|entry| match entry.pick {
                Pick::First => Some(1),
                Pick::Nth(nth) => Some(nth),
                Pick::Last if lone_last => Some(1),
                Pick::Last => None,
            })
            .collect();
        let mut distinct: Vec<usize> = nths.iter().flatten().copied().collect();
        distinct.sort_unstable();
        distinct.dedup();
        let gap = distinct
            .iter()
            .enumerate()
            .find(|(i, nth)| **nth != i + 1)
            .map(|(_, nth)| Some(*nth));
        let broken = nths.iter().position(Option::is_none).or_else(|| {
            let gap = gap.flatten()?;
            nths.iter().position(|nth| *nth == Some(gap))
        });
        match broken.and_then(|at| entries.get(at)) {
            None => Some(nths.into_iter().flatten().collect()),
            Some(entry) => {
                let mut picks: Vec<String> =
                    entries.iter().map(|entry| entry.pick.to_string()).collect();
                picks.sort();
                picks.dedup();
                self.refusals.push(Refusal::PickNotContiguous {
                    table: self.table_name(entry.table),
                    column: self.column_name(entry.table, entry.column),
                    loop_name: self
                        .spec
                        .loops()
                        .get(id)
                        .map(|def| def.name.clone())
                        .unwrap_or_default(),
                    occurrence: occurrence.name.clone(),
                    picks,
                });
                None
            }
        }
    }
}
