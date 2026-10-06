//! The codes that place a column's segment: the occurrence its segment and
//! `where` select, and the refusal of a fixed code that a code list of the
//! written segment does not allow.

use crate::spec::{LoopId, OccurrenceDef};

use super::build::{Builder, Codes};
use super::plan::{SegmentPlan, ValueSource};
use super::refusal::{Refusal, place};

/// Where a segment and its `where` codes land in a loop.
pub(super) enum Resolved {
    /// One occurrence, with the codes left once its single-code qualifier is
    /// taken out.
    One(usize, Codes),
    /// No occurrence: the segment's own code list excludes a `where` code
    /// that an occurrence would otherwise take, so no valid file holds it.
    Nothing,
    /// No occurrence for another reason, or several: their names.
    Candidates(Vec<String>),
}

impl Builder<'_> {
    /// The occurrence of loop `id` that `segment` and `conditions` select.
    ///
    /// A condition on the occurrence's qualifier element must be one of its
    /// codes; on another element, one of the occurrence's codes for it, or,
    /// when the occurrence lists none, one of the element's own codes.
    pub(super) fn resolve(
        &self,
        id: LoopId,
        segment: &[u8],
        conditions: &[(usize, Vec<u8>)],
    ) -> Resolved {
        let spec = self.spec;
        let def = spec.get(id);
        let accepts = |occurrence: &OccurrenceDef, own_codes: bool| {
            occurrence.segment == segment
                && conditions.iter().all(|(position, value)| {
                    let listed = |codes: &[String]| codes.iter().any(|c| c.as_bytes() == value);
                    match &occurrence.qualifier {
                        Some(q) if q.element == *position && q.component.is_none() => {
                            listed(&q.codes)
                        }
                        _ => match occurrence.codes.get(&(*position, None)) {
                            Some(codes) => listed(codes),
                            None => {
                                !own_codes
                                    || spec
                                        .element_def(segment, *position, None)
                                        .is_none_or(|element| !element.rejects_code(value))
                            }
                        },
                    }
                })
        };
        let matched: Vec<usize> = def
            .occurrences
            .iter()
            .enumerate()
            .filter(|(_, occurrence)| accepts(occurrence, true))
            .map(|(index, _)| index)
            .collect();
        let [index] = matched.as_slice() else {
            if matched.is_empty() && def.occurrences.iter().any(|o| accepts(o, false)) {
                return Resolved::Nothing;
            }
            return Resolved::Candidates(
                matched
                    .iter()
                    .filter_map(|&index| def.occurrences.get(index))
                    .map(|occurrence| occurrence.name.clone())
                    .collect(),
            );
        };
        let implied = def
            .occurrences
            .get(*index)
            .and_then(|o| o.qualifier.as_ref())
            .filter(|q| q.component.is_none() && q.codes.len() == 1)
            .map(|q| q.element);
        let rest = conditions
            .iter()
            .filter(|(position, _)| Some(*position) != implied)
            .cloned()
            .collect();
        Resolved::One(*index, rest)
    }

    /// Refuses every fixed code of `plan`, a segment of loop `id` written
    /// from `table`, that the occurrence's code list for its element (its
    /// qualifier's codes, or its own codes) or the element's code list does
    /// not allow. `column` names the column that carries the segment.
    pub(super) fn check_codes(
        &mut self,
        id: usize,
        plan: &SegmentPlan,
        table: usize,
        column: Option<usize>,
    ) {
        let spec = self.spec;
        let Some(def) = spec.loops().get(id) else {
            return;
        };
        let Some(occurrence) = def.occurrences.get(plan.occurrence) else {
            return;
        };
        for element in &plan.elements {
            let ValueSource::Code(value) = &element.value else {
                continue;
            };
            let at = (element.element, element.component);
            let own = match &occurrence.qualifier {
                Some(q) if (q.element, q.component) == at => Some(&q.codes),
                _ => occurrence.codes.get(&at),
            };
            let general = spec
                .element_def(&occurrence.segment, element.element, element.component)
                .map(|element| &element.codes);
            let rejecting = [own, general]
                .into_iter()
                .flatten()
                .find(|codes| !codes.is_empty() && !codes.iter().any(|c| c.as_bytes() == value));
            if let Some(codes) = rejecting {
                self.refusals.push(Refusal::CodeOutsideList {
                    table: self.table_name(table),
                    column: column.map(|column| self.column_name(table, column)),
                    loop_name: def.name.clone(),
                    occurrence: occurrence.name.clone(),
                    place: place(&occurrence.segment, element.element, element.component),
                    value: String::from_utf8_lossy(value).into_owned(),
                    codes: codes.clone(),
                });
            }
        }
    }
}
