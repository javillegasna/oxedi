//! The compilation of the code lists of a loop's occurrences against the
//! `segments` section: the qualifier's list and each own list, checked with
//! the rules an element's own `codes` gets.

use std::collections::BTreeMap;

use super::occurrence_error::OccurrenceError;
use super::segments::compile_codes;
use super::vocabulary::ElementPosition;
use crate::message::place;
use crate::spec::{ElementDef, Qualifier, SegmentDef, element_def};

/// A code list of an occurrence and the element or component it is for.
pub(super) struct CodeList<'r> {
    /// Where the element reference sits: `qualifier` or `codes.<key>`.
    pub(super) reference: String,
    /// Where the list sits: `qualifier.codes` or `codes.<key>`.
    pub(super) list: String,
    /// 1-based element position.
    pub(super) element: usize,
    /// 1-based component position, when the list is for a component.
    pub(super) component: Option<usize>,
    pub(super) codes: &'r [String],
}

/// Finds the element a code list is for, refuses an empty list and validates
/// the list with the checks an element's own `codes` gets: the element's
/// definition and the list sorted by bytes; no code may be one the element's
/// own `codes` leaves out.
pub(super) fn compile_list<'d>(
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
    let codes = compile_codes(codes, def.kind, def.length, !def.composite.is_empty()).map_err(
        |reason| OccurrenceError::BadCodes {
            key: key.clone(),
            reason,
        },
    )?;
    if let Some(code) = outside(def, &codes) {
        return Err(OccurrenceError::CodeOutsideElement {
            key,
            code: code.clone(),
            place: place(segment, element, component),
            allowed: def.codes.clone(),
        });
    }
    Ok((def, codes))
}

/// The first of `codes` that the element's own `codes` leaves out.
fn outside<'c>(target: &ElementDef, codes: &'c [String]) -> Option<&'c String> {
    codes
        .iter()
        .find(|code| target.rejects_code(code.as_bytes()))
}

/// Code lists by `(element, component)`, each sorted by bytes.
type OwnCodes = BTreeMap<(usize, Option<usize>), Vec<String>>;

/// The occurrence's own code lists by `(element, component)`; none may sit at
/// the qualifier's place.
pub(super) fn compile_own_codes(
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
        let (_, list) = compile_list(segments, segment, list)?;
        codes.insert((element, component), list);
    }
    Ok(codes)
}
