//! The codes that place a column's segment: the occurrence its compiled place
//! lands in, the conditions left once its qualifier is taken out, and the
//! refusal of a fixed qualifier code that the element's code list does not
//! allow.

use crate::spec::{ElementRule, LoopDef, OccurrenceDef, OccurrencePlace, Unplaced};

use super::build::{Builder, Codes};
use super::plan::{SegmentPlan, ValueSource};
use super::refusal::Refusal;
use crate::message::place;

/// The occurrence of `def` a column's compiled `place` writes `segment` in:
/// the one it names or chose, or the one occurrence of `segment` when the
/// place cannot tell several apart. The `Err` holds the names of the
/// occurrences it could be (empty when the loop has none for it).
pub(super) fn landing(
    def: &LoopDef,
    place: OccurrencePlace,
    segment: &[u8],
) -> Result<usize, Vec<String>> {
    match place {
        OccurrencePlace::Named(occurrence) | OccurrencePlace::Chosen(occurrence) => {
            Ok(occurrence.index())
        }
        OccurrencePlace::Unplaced(Unplaced::QualifierFree) => {
            let taking: Vec<(usize, &OccurrenceDef)> = def
                .occurrences
                .iter()
                .enumerate()
                .filter(|(_, occurrence)| occurrence.segment == segment)
                .collect();
            match taking.as_slice() {
                [(at, _)] => Ok(*at),
                several => Err(several.iter().map(|(_, o)| o.name.clone()).collect()),
            }
        }
        OccurrencePlace::Unplaced(Unplaced::NoOccurrence | Unplaced::NoOccurrenceTakes) => {
            Err(Vec::new())
        }
    }
}

impl Builder<'_> {
    /// Refuses the fixed code of `plan`, a segment of loop `id` written from
    /// `table`, at its occurrence's qualifier place when the element rule
    /// there (the occurrence's own list, else the element's) does not allow
    /// it. The codes of a column's `where` are not checked: they are the
    /// place the column is read from, and reading the file back reports them.
    /// `column` names the column that carries the segment.
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
        let Some(qualifier) = &occurrence.qualifier else {
            return;
        };
        let Some(definition) =
            spec.element_def(&occurrence.segment, qualifier.element, qualifier.component)
        else {
            return;
        };
        let rule = ElementRule::at(
            definition,
            Some(occurrence),
            qualifier.element,
            qualifier.component,
        );
        let fixed = plan
            .elements
            .iter()
            .filter(|e| e.element == qualifier.element && e.component == qualifier.component);
        for element in fixed {
            let ValueSource::Code(value) = &element.value else {
                continue;
            };
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
pub(super) fn implied_rest(def: &LoopDef, index: usize, conditions: &[(usize, Vec<u8>)]) -> Codes {
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
