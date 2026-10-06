//! What a written loop requires: its required child loops, occurrences and
//! elements, the single codes that fill an element no column writes, and the
//! refusals for the rest.

use crate::spec::{OccurrenceDef, Spec, Usage, render_selector, render_trigger};

use super::build::Builder;
use super::plan::{ElementPlan, Instances, SegmentPlan, SegmentSource, ValueSource};
use super::refusal::{Refusal, place};

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
    let taken = plan
        .elements
        .iter()
        .any(|e| (e.element, e.component) == (qualifier.element, qualifier.component));
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
    let covered = |segment: &SegmentPlan, element: usize, component: Option<usize>| {
        segment
            .elements
            .iter()
            .any(|e| e.element == element && (component.is_none() || e.component == component))
    };
    let mut wanted = Vec::new();
    for (&position, element) in &def.elements {
        if element.composite.is_empty() {
            if element.required {
                wanted.push((position, None, element));
            }
            continue;
        }
        let written = covered(segment, position, None);
        if element.required || written {
            for (&component, part) in &element.composite {
                if part.required {
                    wanted.push((position, Some(component), part));
                }
            }
        }
    }
    for (position, component, element) in wanted {
        if covered(segment, position, component) {
            continue;
        }
        let single = occurrence
            .codes
            .get(&(position, component))
            .or((!element.codes.is_empty()).then_some(&element.codes))
            .and_then(|codes| match codes.as_slice() {
                [code] => Some(code.as_bytes().to_vec()),
                _ => None,
            });
        match single {
            Some(code) => segment.elements.push(ElementPlan {
                element: position,
                component,
                value: ValueSource::Code(code),
            }),
            None => missing.push((position, component, element.name.clone())),
        }
    }
    missing
}
