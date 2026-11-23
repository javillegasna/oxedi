//! What a written loop requires: its required child loops, occurrences and
//! elements, the single codes that fill an element no column writes, and the
//! refusals for the rest.

use crate::spec::{
    ElementRule, OccurrenceDef, SegmentDef, Spec, Usage, render_selector, render_trigger,
};

use super::build::Builder;
use super::plan::{ElementPlan, Instances, Place, SegmentPlan, SegmentSource, ValueSource};
use super::refusal::Refusal;
use crate::message::place;

impl Builder<'_> {
    /// Refuses what a written loop requires and nothing writes: a child
    /// loop, an occurrence, or an element of a segment it writes. A required
    /// element that allows a single code gets it.
    pub(super) fn required(&mut self) {
        let spec = self.spec;
        for (index, def) in spec.loops().iter().enumerate() {
            let Some(plan) = self.loops.get(index) else {
                continue;
            };
            if plan.instances == Instances::Absent {
                continue;
            }
            let table = match plan.instances {
                Instances::Rows { table }
                | Instances::Inside { table }
                | Instances::Groups { table } => Some(table),
                _ => None,
            };
            for &child in &def.children {
                let child_def = spec.get(child);
                let absent = self
                    .loops
                    .get(child.index())
                    .is_none_or(|plan| plan.instances == Instances::Absent);
                if child_def.usage == Usage::Required && absent {
                    self.refusals.push(Refusal::RequiredLoopWithoutSource {
                        loop_name: child_def.name.clone(),
                        parent: def.name.clone(),
                        trigger: render_trigger(&child_def.trigger),
                    });
                }
            }
            for (at, occurrence) in def.occurrences.iter().enumerate() {
                let written = self
                    .loops
                    .get(index)
                    .is_some_and(|plan| plan.segments.iter().any(|s| s.occurrence == at));
                if occurrence.usage == Usage::Required && !written {
                    self.refusals
                        .push(Refusal::RequiredOccurrenceWithoutSource {
                            loop_name: def.name.clone(),
                            occurrence: occurrence.name.clone(),
                            selector: render_selector(occurrence),
                        });
                }
            }
            let mut found = Vec::new();
            if let Some(plan) = self.loops.get_mut(index) {
                for segment in &mut plan.segments {
                    let segment_table = match segment.source {
                        SegmentSource::Envelope => continue,
                        SegmentSource::Row { table, .. } | SegmentSource::Repeat { table } => {
                            Some(table)
                        }
                    };
                    let Some(occurrence) = def.occurrences.get(segment.occurrence) else {
                        continue;
                    };
                    for (element, component, name) in fill_required(spec, segment, occurrence) {
                        found.push(Refusal::RequiredElementWithoutSource {
                            table: segment_table.or(table).map(|t| {
                                spec.tables()
                                    .get(t)
                                    .map(|d| d.name.clone())
                                    .unwrap_or_default()
                            }),
                            loop_name: def.name.clone(),
                            occurrence: occurrence.name.clone(),
                            place: place(&occurrence.segment, element, component),
                            name,
                        });
                    }
                }
            }
            self.refusals.extend(found);
        }
    }
}

/// Adds the occurrence's qualifier code when it has exactly one and nothing
/// writes that element yet.
pub(super) fn add_qualifier(plan: &mut SegmentPlan, occurrence: &OccurrenceDef) {
    let Some(qualifier) = &occurrence.qualifier else {
        return;
    };
    let [code] = qualifier.codes.as_slice() else {
        return;
    };
    let place = Place {
        element: qualifier.element,
        component: qualifier.component,
    };
    let taken = plan.elements.iter().any(|e| e.place().overlaps(place));
    if !taken {
        plan.elements.push(ElementPlan {
            element: qualifier.element,
            component: qualifier.component,
            value: ValueSource::Code(code.as_bytes().to_vec()),
        });
    }
}

/// Gives each required element (or required component of a composite the
/// segment writes or requires) that has no source its single allowed code,
/// and returns the ones that allow none or several.
fn fill_required(
    spec: &Spec,
    segment: &mut SegmentPlan,
    occurrence: &OccurrenceDef,
) -> Vec<(usize, Option<usize>, String)> {
    let mut missing = Vec::new();
    let Some(def) = spec.segment(&occurrence.segment) else {
        return missing;
    };
    for (place, rule) in required_places(def, segment, occurrence) {
        if is_written(segment, place) {
            continue;
        }
        match single_code(rule) {
            Some(code) => segment.elements.push(ElementPlan {
                element: place.element,
                component: place.component,
                value: ValueSource::Code(code),
            }),
            None => missing.push((place.element, place.component, rule.def.name.clone())),
        }
    }
    missing
}

/// Whether some element or component the segment plan writes overlaps `place`.
fn is_written(segment: &SegmentPlan, place: Place) -> bool {
    segment.elements.iter().any(|e| e.place().overlaps(place))
}

/// The places the segment requires, in element then component order, with
/// the rule `occurrence` gives each: a required simple element, and each
/// required component of a composite that is required or that the plan
/// already writes.
fn required_places<'a>(
    def: &'a SegmentDef,
    segment: &SegmentPlan,
    occurrence: &'a OccurrenceDef,
) -> Vec<(Place, ElementRule<'a>)> {
    let mut places = Vec::new();
    for (&element, definition) in &def.elements {
        let whole = Place {
            element,
            component: None,
        };
        let rule = ElementRule::at(definition, Some(occurrence), element, None);
        if definition.composite.is_empty() {
            if rule.required {
                places.push((whole, rule));
            }
        } else if rule.required || is_written(segment, whole) {
            places.extend(
                definition
                    .composite
                    .iter()
                    .filter_map(|(&component, part)| {
                        let place = Place {
                            element,
                            component: Some(component),
                        };
                        let rule =
                            ElementRule::at(part, Some(occurrence), element, Some(component));
                        rule.required.then_some((place, rule))
                    }),
            );
        }
    }
    places
}

/// The one code a place allows: the rule's list, the occurrence's own or
/// else the element's; `None` when there are none or several.
fn single_code(rule: ElementRule<'_>) -> Option<Vec<u8>> {
    match rule.codes {
        [code] => Some(code.as_bytes().to_vec()),
        _ => None,
    }
}
