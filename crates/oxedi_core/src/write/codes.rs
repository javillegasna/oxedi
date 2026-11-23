//! The codes that place a column's segment: the occurrence its segment and
//! `where` select, and the refusal of a fixed code that a code list of the
//! written segment does not allow.

use crate::spec::{ElementRule, LoopDef, LoopId, OccurrenceDef, holds_code};

use super::build::{Builder, Codes};
use super::plan::{SegmentPlan, ValueSource};
use super::refusal::Refusal;
use crate::message::place;

/// A `where` code the element's own code list excludes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Excluded {
    /// The element, e.g. `PER01`.
    pub(super) place: String,
    /// The excluded code.
    pub(super) code: String,
    /// The element's code list.
    pub(super) codes: Vec<String>,
}

/// Where a segment and its `where` codes land in a loop.
pub(super) enum Resolved {
    /// One occurrence, with the codes left once its single-code qualifier is
    /// taken out.
    One(usize, Codes),
    /// No occurrence: the segment's own code list excludes a `where` code
    /// that an occurrence would otherwise take, so no valid file holds it.
    Nothing(Excluded),
    /// No occurrence for another reason, or several: their names.
    Candidates(Vec<String>),
}

/// Which code lists a `where` code must meet to select an occurrence.
#[derive(Clone, Copy)]
enum Lists {
    /// The rule's list at the place: the occurrence's own, else the element's.
    All,
    /// The occurrence's own list only; an element's list is not asked.
    Own,
}

impl Lists {
    /// `true` when the lists this pass asks reject `value` at `rule`'s place.
    fn rejects(self, rule: &ElementRule<'_>, value: &[u8]) -> bool {
        match self {
            Self::All => rule.rejects(value),
            Self::Own => rule.own_rejects(value),
        }
    }
}

impl Builder<'_> {
    /// The occurrence of loop `id` that `segment` and `conditions` select.
    ///
    /// A condition on the occurrence's qualifier element must be one of its
    /// codes; on another element, it must pass the element rule there: the
    /// occurrence's own codes for it, or, when it lists none, the element's.
    pub(super) fn resolve(
        &self,
        id: LoopId,
        segment: &[u8],
        conditions: &[(usize, Vec<u8>)],
    ) -> Resolved {
        let def = self.spec.get(id);
        let matched: Vec<usize> = def
            .occurrences
            .iter()
            .enumerate()
            .filter(|(_, occurrence)| self.accepts(occurrence, segment, conditions, Lists::All))
            .map(|(index, _)| index)
            .collect();
        match matched.as_slice() {
            [index] => Resolved::One(*index, implied_rest(def, *index, conditions)),
            [] => self
                .excluded(def, segment, conditions)
                .map_or(Resolved::Candidates(Vec::new()), Resolved::Nothing),
            several => Resolved::Candidates(
                several
                    .iter()
                    .filter_map(|&index| def.occurrences.get(index))
                    .map(|occurrence| occurrence.name.clone())
                    .collect(),
            ),
        }
    }

    /// Whether `occurrence` takes `segment` with `conditions`: a condition on
    /// its qualifier's element must be one of the qualifier's codes, which
    /// select it; any other must pass the element rule at its place, over
    /// the code lists `lists` names.
    fn accepts(
        &self,
        occurrence: &OccurrenceDef,
        segment: &[u8],
        conditions: &[(usize, Vec<u8>)],
        lists: Lists,
    ) -> bool {
        occurrence.segment == segment
            && conditions
                .iter()
                .all(|(position, value)| match &occurrence.qualifier {
                    Some(q) if q.element == *position && q.component.is_none() => {
                        holds_code(&q.codes, value)
                    }
                    _ => self
                        .spec
                        .element_def(segment, *position, None)
                        .is_none_or(|def| {
                            let rule = ElementRule::at(def, Some(occurrence), *position, None);
                            !lists.rejects(&rule, value)
                        }),
                })
    }

    /// The first condition the element's own list excludes, when some
    /// occurrence would take the segment with the element lists left out: no
    /// valid file holds it.
    fn excluded(
        &self,
        def: &LoopDef,
        segment: &[u8],
        conditions: &[(usize, Vec<u8>)],
    ) -> Option<Excluded> {
        def.occurrences
            .iter()
            .any(|o| self.accepts(o, segment, conditions, Lists::Own))
            .then(|| {
                conditions.iter().find_map(|(position, value)| {
                    let element = self.spec.element_def(segment, *position, None)?;
                    element.rejects_code(value).then(|| Excluded {
                        place: place(segment, *position, None),
                        code: String::from_utf8_lossy(value).into_owned(),
                        codes: element.codes.clone(),
                    })
                })
            })
            .flatten()
    }

    /// Refuses every fixed code of `plan`, a segment of loop `id` written
    /// from `table`, that the element rule at its place (the occurrence's own
    /// list, else the element's) does not allow. `column` names the column
    /// that carries the segment.
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
            let Some(definition) =
                spec.element_def(&occurrence.segment, element.element, element.component)
            else {
                continue;
            };
            let rule = ElementRule::at(
                definition,
                Some(occurrence),
                element.element,
                element.component,
            );
            if rule.rejects(value) {
                self.refusals.push(Refusal::CodeOutsideList {
                    table: self.table_name(table),
                    column: column.map(|column| self.column_name(table, column)),
                    loop_name: def.name.clone(),
                    occurrence: occurrence.name.clone(),
                    place: place(&occurrence.segment, element.element, element.component),
                    value: String::from_utf8_lossy(value).into_owned(),
                    codes: rule.codes.to_vec(),
                });
            }
        }
    }
}

/// The conditions left once the occurrence's single-code qualifier is taken
/// out: that code is written for the qualifier, not by a column.
fn implied_rest(def: &LoopDef, index: usize, conditions: &[(usize, Vec<u8>)]) -> Codes {
    let implied = def
        .occurrences
        .get(index)
        .and_then(|o| o.qualifier.as_ref())
        .filter(|q| q.component.is_none() && q.codes.len() == 1)
        .map(|q| q.element);
    conditions
        .iter()
        .filter(|(position, _)| Some(*position) != implied)
        .cloned()
        .collect()
}
