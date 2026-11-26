//! The compilation of a loop's occurrences against the `segments` section: each
//! occurrence's own keys and qualifier, the check that every two
//! occurrences of one segment can be told apart, and the check of the
//! occurrence the loop's trigger opens on.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use super::build_entries::{CodeList, compile_entries, compile_list, find_element};
use super::error::SpecError;
use super::model::{OccurrenceModel, QualifierModel};
use super::occurrence_error::OccurrenceError;
use super::vocabulary::ElementPosition;
use crate::message::{place, render_key};
use crate::spec::{OccurrenceDef, Qualifier, SegmentDef, Selector, Trigger};

/// Checks the occurrence a loop's trigger opens on, when the loop declares
/// occurrences: there is one, its qualifier selects no code the trigger's
/// conditions leave out, and it comes before every other occurrence.
pub(super) fn check_trigger(
    loop_name: &str,
    occurrences: &[OccurrenceDef],
    trigger: &Trigger,
) -> Result<(), SpecError> {
    if occurrences.is_empty() {
        return Ok(());
    }
    let Some(first) = occurrences.iter().find(|o| o.opens_on(trigger)) else {
        return Err(SpecError::UnmatchedTrigger {
            loop_name: loop_name.to_string(),
            trigger: Selector::trigger(trigger),
            segment_held: occurrences.iter().any(|o| o.segment == trigger.segment),
        });
    };
    if let Some(qualifier) = &first.qualifier {
        let selected = trigger
            .conditions
            .iter()
            .find(|(position, _)| *position == qualifier.element)
            .map(|(_, value)| value.as_slice());
        if let Some(code) = qualifier
            .codes
            .iter()
            .find(|code| Some(code.as_bytes()) != selected)
        {
            return Err(SpecError::TriggerQualifierWider {
                loop_name: loop_name.to_string(),
                occurrence: first.name.clone(),
                trigger: Box::new(Selector::trigger(trigger)),
                code: code.clone(),
            });
        }
    }
    if let Some(other) = occurrences
        .iter()
        .find(|o| o.name != first.name && o.pos <= first.pos)
    {
        return Err(SpecError::TriggerNotFirst {
            loop_name: loop_name.to_string(),
            occurrence: first.name.clone(),
            pos: first.pos,
            other: other.name.clone(),
            other_pos: other.pos,
        });
    }
    Ok(())
}

/// The occurrences of one segment seen so far in a loop, in position order:
/// the first, whose qualifier place every later one must share, and the
/// occurrence that owns each qualifier code already taken.
struct SegmentSeen<'o> {
    first: &'o OccurrenceDef,
    /// Qualifier code -> name of the occurrence whose qualifier accepts it.
    owners: BTreeMap<&'o str, &'o str>,
}

impl<'o> SegmentSeen<'o> {
    /// Seeds `owners` with `first`'s qualifier codes (none when it has no
    /// qualifier). One code list holds no duplicate, so no code collides here.
    fn new(first: &'o OccurrenceDef) -> Self {
        let owners = first
            .qualifier
            .iter()
            .flat_map(|qualifier| &qualifier.codes)
            .map(|code| (code.as_str(), first.name.as_str()))
            .collect();
        Self { first, owners }
    }

    /// Checks a later occurrence of the same segment and takes its codes. An
    /// admitted occurrence reads its qualifier where `first` does, so the same
    /// place is transitive through `first`, and codes disjoint pairwise is no
    /// code owned twice.
    fn admit(&mut self, occurrence: &'o OccurrenceDef) -> Result<(), OccurrenceError> {
        let segment = String::from_utf8_lossy(&occurrence.segment).into_owned();
        let (Some(a), Some(b)) = (&self.first.qualifier, &occurrence.qualifier) else {
            return Err(OccurrenceError::Indistinct {
                other: self.first.name.clone(),
                segment,
            });
        };
        if (a.element, a.component) != (b.element, b.component) {
            return Err(OccurrenceError::QualifierPlaces {
                other: self.first.name.clone(),
                place: place(&segment, b.element, b.component),
                other_place: place(&segment, a.element, a.component),
            });
        }
        for code in &b.codes {
            if let Some(owner) = self.owners.insert(code, &occurrence.name) {
                return Err(OccurrenceError::SharedCode {
                    other: owner.to_string(),
                    segment,
                    code: code.clone(),
                });
            }
        }
        Ok(())
    }
}

/// The error of an occurrence of `loop_name`, wrapping its reason.
fn bad_occurrence(loop_name: &str, occurrence: &str, reason: OccurrenceError) -> SpecError {
    SpecError::BadOccurrence {
        loop_name: loop_name.to_string(),
        occurrence: occurrence.to_string(),
        reason: Box::new(reason),
    }
}

/// Checks that every two occurrences of one segment can be told apart, in one
/// pass over `occurrences` (sorted by position).
fn check_distinct(loop_name: &str, occurrences: &[OccurrenceDef]) -> Result<(), SpecError> {
    let mut seen: BTreeMap<&[u8], SegmentSeen<'_>> = BTreeMap::new();
    for occurrence in occurrences {
        match seen.entry(occurrence.segment.as_slice()) {
            Entry::Vacant(slot) => {
                slot.insert(SegmentSeen::new(occurrence));
            }
            Entry::Occupied(mut slot) => slot
                .get_mut()
                .admit(occurrence)
                .map_err(|reason| bad_occurrence(loop_name, &occurrence.name, reason))?,
        }
    }
    Ok(())
}

/// Compiles the occurrences of the loop `loop_name` against the segment
/// definitions, sorted by position (ties keep name order), and checks that
/// every two occurrences of one segment can be told apart.
pub(super) fn compile_occurrences(
    loop_name: &str,
    raw: &BTreeMap<String, OccurrenceModel>,
    segments: &BTreeMap<Vec<u8>, SegmentDef>,
) -> Result<Vec<OccurrenceDef>, SpecError> {
    let mut occurrences = raw
        .iter()
        .map(|(name, def)| compile_occurrence(loop_name, name, def, segments))
        .collect::<Result<Vec<_>, _>>()?;
    occurrences.sort_by_key(|occurrence| occurrence.pos);
    check_distinct(loop_name, &occurrences)?;
    Ok(occurrences)
}

/// A check of an occurrence's own keys: the error when the occurrence breaks it.
type OccurrenceRule = fn(&str, &str, &OccurrenceModel) -> Option<SpecError>;

/// The checks of an occurrence's keys that need nothing but the occurrence, in
/// the order they run.
const OCCURRENCE_RULES: [OccurrenceRule; 3] = [empty_name, empty_segment, zero_max];

fn empty_name(loop_name: &str, name: &str, _def: &OccurrenceModel) -> Option<SpecError> {
    name.is_empty()
        .then(|| bad_occurrence(loop_name, name, OccurrenceError::EmptyName))
}

fn empty_segment(loop_name: &str, name: &str, def: &OccurrenceModel) -> Option<SpecError> {
    def.segment.is_empty().then(|| SpecError::EmptySegmentId {
        loop_name: Some(loop_name.to_string()),
        key: format!("occurrences.{}.segment", render_key(name)),
    })
}

fn zero_max(loop_name: &str, name: &str, def: &OccurrenceModel) -> Option<SpecError> {
    (def.max == Some(0)).then(|| bad_occurrence(loop_name, name, OccurrenceError::ZeroMax))
}

/// The qualifier: its code list, then no code its element's own `codes`
/// leaves out.
fn compile_qualifier(
    segments: &BTreeMap<Vec<u8>, SegmentDef>,
    segment: &str,
    raw: &QualifierModel,
) -> Result<Qualifier, OccurrenceError> {
    let at = ElementPosition {
        element: raw.element,
        component: raw.component,
    };
    let def = find_element(segments, segment, "qualifier", at)?;
    let list = CodeList {
        list: "qualifier.codes".into(),
        element: raw.element,
        component: raw.component,
        codes: &raw.codes,
    };
    let codes = compile_list(def, segment, list, def.length)?;
    Ok(Qualifier {
        element: raw.element,
        component: raw.component,
        codes,
    })
}

fn compile_occurrence(
    loop_name: &str,
    name: &str,
    def: &OccurrenceModel,
    segments: &BTreeMap<Vec<u8>, SegmentDef>,
) -> Result<OccurrenceDef, SpecError> {
    if let Some(error) = OCCURRENCE_RULES
        .iter()
        .find_map(|rule| rule(loop_name, name, def))
    {
        return Err(error);
    }
    let fail = |reason| bad_occurrence(loop_name, name, reason);
    let qualifier = def
        .qualifier
        .as_ref()
        .map(|q| compile_qualifier(segments, &def.segment, q))
        .transpose()
        .map_err(fail)?;
    let elements =
        compile_entries(segments, &def.segment, &def.elements, qualifier.as_ref()).map_err(fail)?;
    Ok(OccurrenceDef {
        name: name.to_string(),
        segment: def.segment.as_bytes().to_vec(),
        pos: def.pos,
        usage: def.usage,
        max: def.max,
        qualifier,
        elements,
    })
}
