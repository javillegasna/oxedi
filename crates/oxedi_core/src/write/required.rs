//! What a written loop requires: its required child loops, occurrences and
//! elements, the single codes that fill an element no column writes, and the
//! refusals for the rest.

use crate::spec::{ElementRule, LoopDef, OccurrenceDef, SegmentDef, Selector, Spec, Usage};

use super::build::Builder;
use super::plan::{ElementPlan, Instances, Place, SegmentPlan, ValueSource};
use super::refusal::Refusal;
use crate::message::ElementRef;

impl Builder<'_> {
    /// Refuses what a written loop requires and nothing writes: a child
    /// loop, an occurrence, or an element of a segment it writes. A required
    /// element that allows a single code gets it.
    pub(super) fn required(&mut self) {
        let spec = self.spec;
        for (index, def) in spec.loops().iter().enumerate() {
            if self.is_absent(index) {
                continue;
            }
            let unwritten: Vec<Refusal> = self
                .children_without_source(def)
                .chain(self.occurrences_without_source(index, def))
                .collect();
            self.refusals.extend(unwritten);
            let unfilled = self.fill_elements(index, def);
            self.refusals.extend(unfilled);
        }
    }

    /// Whether loop `index` has no plan or an absent one.
    fn is_absent(&self, index: usize) -> bool {
        self.loops
            .get(index)
            .is_none_or(|plan| plan.instances == Instances::Absent)
    }

    /// The required child loops of `def` that nothing writes.
    fn children_without_source<'a>(
        &'a self,
        def: &'a LoopDef,
    ) -> impl Iterator<Item = Refusal> + 'a {
        def.children
            .iter()
            .filter(|&&child| self.spec.get(child).usage == Usage::Required)
            .filter(|&&child| self.is_absent(child.index()))
            .map(|&child| self.spec.get(child))
            .map(|child| Refusal::RequiredLoopWithoutSource {
                loop_name: child.name.clone(),
                parent: def.name.clone(),
                trigger: Selector::trigger(&child.trigger),
            })
    }

    /// The required occurrences of loop `index` that no segment writes.
    fn occurrences_without_source<'a>(
        &'a self,
        index: usize,
        def: &'a LoopDef,
    ) -> impl Iterator<Item = Refusal> + 'a {
        let segments = self.loops.get(index).map_or(&[][..], |plan| &plan.segments);
        def.occurrences
            .iter()
            .enumerate()
            .filter(|(_, occurrence)| occurrence.usage == Usage::Required)
            .filter(move |(at, _)| segments.iter().all(|s| s.occurrence != *at))
            .map(|(_, occurrence)| Refusal::RequiredOccurrenceWithoutSource {
                loop_name: def.name.clone(),
                occurrence: occurrence.name.clone(),
                selector: Selector::occurrence(occurrence),
            })
    }

    /// Gives the required elements of the segments loop `index` writes their
    /// single allowed code, and refuses the ones that allow none or several.
    fn fill_elements(&mut self, index: usize, def: &LoopDef) -> Vec<Refusal> {
        let spec = self.spec;
        let Some(plan) = self.loops.get_mut(index) else {
            return Vec::new();
        };
        let mut refusals = Vec::new();
        for segment in &mut plan.segments {
            let Some(segment_table) = segment.source.table() else {
                continue;
            };
            let Some(occurrence) = def.occurrences.get(segment.occurrence) else {
                continue;
            };
            let table = table_name(spec, segment_table);
            refusals.extend(fill_required(spec, segment, occurrence).into_iter().map(
                |(element, component, name)| Refusal::RequiredElementWithoutSource {
                    table: Some(table.clone()),
                    loop_name: def.name.clone(),
                    occurrence: occurrence.name.clone(),
                    place: ElementRef::new(occurrence.segment.clone(), element, component),
                    name,
                },
            ));
        }
        refusals
    }
}

/// The name of the table at an index; an index the spec does not hold reads as an
/// empty name.
fn table_name(spec: &Spec, table: usize) -> String {
    spec.tables()
        .get(table)
        .map_or_else(String::new, |def| def.name.clone())
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
