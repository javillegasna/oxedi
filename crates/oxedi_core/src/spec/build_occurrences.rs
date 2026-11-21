//! The compilation of a loop's occurrences against the `segments` section: each
//! occurrence's own keys, qualifier and code lists, the check that every two
//! occurrences of one segment can be told apart, and the check of the
//! occurrence the loop's trigger opens on.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use super::error::SpecError;
use super::loops::Trigger;
use super::model::{OccurrenceModel, QualifierModel};
use super::occurrence_error::OccurrenceError;
use super::occurrences::{OccurrenceDef, Qualifier, element_def};
use super::render::render_trigger;
use super::segments::{ElementDef, SegmentDef, compile_codes};
use super::vocabulary::ElementPosition;
use crate::message::{place, render_key};

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
            trigger: render_trigger(trigger),
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
                trigger: render_trigger(trigger),
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

/// A code list of an occurrence and the element or component it is for.
struct CodeList<'r> {
    /// Where the element reference sits: `qualifier` or `codes.<key>`.
    reference: String,
    /// Where the list sits: `qualifier.codes` or `codes.<key>`.
    list: String,
    /// 1-based element position.
    element: usize,
    /// 1-based component position, when the list is for a component.
    component: Option<usize>,
    codes: &'r [String],
}

/// Finds the element a code list is for, refuses an empty list and validates
/// the list with the checks an element's own `codes` gets: the element's
/// definition and the list sorted by bytes.
fn compile_list<'d>(
    segments: &'d BTreeMap<Vec<u8>, SegmentDef>,
    segment: &str,
    list: CodeList<'_>,
) -> Result<(&'d ElementDef, Vec<String>), OccurrenceError> {
    let CodeList {
        reference,
        list: key,
        element,
        component,
        codes,
    } = list;
    let def = element_def(segments, segment.as_bytes(), element, component).ok_or_else(|| {
        OccurrenceError::UndefinedElement {
            key: reference,
            segment: segment.to_string(),
            element,
            component,
        }
    })?;
    if codes.is_empty() {
        return Err(OccurrenceError::EmptyCodes { key });
    }
    let codes = compile_codes(codes, def.kind, def.length, !def.composite.is_empty())
        .map_err(|reason| OccurrenceError::BadCodes { key, reason })?;
    Ok((def, codes))
}

/// The qualifier: its code list, then no code its element's own `codes`
/// leaves out.
fn compile_qualifier(
    segments: &BTreeMap<Vec<u8>, SegmentDef>,
    segment: &str,
    raw: &QualifierModel,
) -> Result<Qualifier, OccurrenceError> {
    let list = CodeList {
        reference: "qualifier".into(),
        list: "qualifier.codes".into(),
        element: raw.element,
        component: raw.component,
        codes: &raw.codes,
    };
    let (target, codes) = compile_list(segments, segment, list)?;
    if let Some(code) = codes
        .iter()
        .find(|code| target.rejects_code(code.as_bytes()))
    {
        return Err(OccurrenceError::QualifierCodeOutsideElement {
            code: code.clone(),
            place: place(segment, raw.element, raw.component),
            allowed: target.codes.clone(),
        });
    }
    Ok(Qualifier {
        element: raw.element,
        component: raw.component,
        codes,
    })
}

/// Code lists by `(element, component)`, each sorted by bytes.
type OwnCodes = BTreeMap<(usize, Option<usize>), Vec<String>>;

/// The occurrence's own code lists by `(element, component)`; none may sit at
/// the qualifier's place.
fn compile_own_codes(
    segments: &BTreeMap<Vec<u8>, SegmentDef>,
    segment: &str,
    raw: &BTreeMap<ElementPosition, Vec<String>>,
    qualifier: Option<&Qualifier>,
) -> Result<OwnCodes, OccurrenceError> {
    let mut codes = BTreeMap::new();
    for (&at, list) in raw {
        let ElementPosition { element, component } = at;
        if qualifier.is_some_and(|q| (q.element, q.component) == (element, component)) {
            return Err(OccurrenceError::QualifierInCodes {
                key: at.to_string(),
            });
        }
        let key = format!("codes.{at}");
        let list = CodeList {
            reference: key.clone(),
            list: key,
            element,
            component,
            codes: list,
        };
        codes.insert(
            (element, component),
            compile_list(segments, segment, list)?.1,
        );
    }
    Ok(codes)
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
    let codes =
        compile_own_codes(segments, &def.segment, &def.codes, qualifier.as_ref()).map_err(fail)?;
    Ok(OccurrenceDef {
        name: name.to_string(),
        segment: def.segment.as_bytes().to_vec(),
        pos: def.pos,
        usage: def.usage,
        max: def.max,
        qualifier,
        codes,
    })
}
