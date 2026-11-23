//! The compilation of the element entries of a loop's occurrences against the
//! `segments` section: the qualifier's code list and, per entry, the codes it
//! lists, checked with the rules an element's own `codes` gets.

use std::collections::BTreeMap;

use super::model::ElementEntryModel;
use super::occurrence_error::OccurrenceError;
use super::segments::compile_codes;
use super::vocabulary::ElementPosition;
use crate::message::place;
use crate::spec::{ElementDef, ElementEntry, Qualifier, SegmentDef, Setting, element_def};
use crate::value::LengthRange;

/// A code list of an occurrence and the element or component it is for.
pub(super) struct CodeList<'r> {
    /// Where the list sits: `qualifier.codes` or `elements.<key>.codes`.
    pub(super) list: String,
    /// 1-based element position.
    pub(super) element: usize,
    /// 1-based component position, when the list is for a component.
    pub(super) component: Option<usize>,
    pub(super) codes: &'r [String],
}

/// Finds the element a reference (`qualifier` or `elements.<key>`) names.
pub(super) fn find_element<'d>(
    segments: &'d BTreeMap<Vec<u8>, SegmentDef>,
    segment: &str,
    reference: &str,
    at: ElementPosition,
) -> Result<&'d ElementDef, OccurrenceError> {
    let ElementPosition { element, component } = at;
    element_def(segments, segment.as_bytes(), element, component).ok_or_else(|| {
        OccurrenceError::UndefinedElement {
            key: reference.to_owned(),
            segment: segment.to_string(),
            element,
            component,
        }
    })
}

/// Refuses an empty list and validates the list with the checks an element's
/// own `codes` gets, against `length`: the list sorted by bytes; no code may
/// be one the element's own `codes` leaves out.
pub(super) fn compile_list(
    def: &ElementDef,
    segment: &str,
    list: CodeList<'_>,
    length: LengthRange,
) -> Result<Vec<String>, OccurrenceError> {
    let CodeList {
        list: key,
        element,
        component,
        codes,
    } = list;
    if codes.is_empty() {
        return Err(OccurrenceError::EmptyCodes { key });
    }
    let codes =
        compile_codes(codes, def.kind, length, !def.composite.is_empty()).map_err(|reason| {
            OccurrenceError::BadCodes {
                key: key.clone(),
                reason,
            }
        })?;
    if let Some(code) = outside(def, &codes) {
        return Err(OccurrenceError::CodeOutsideElement {
            key,
            code: code.clone(),
            place: place(segment, element, component),
            allowed: def.codes.clone(),
        });
    }
    Ok(codes)
}

/// The first of `codes` that the element's own `codes` leaves out.
fn outside<'c>(target: &ElementDef, codes: &'c [String]) -> Option<&'c String> {
    codes
        .iter()
        .find(|code| target.rejects_code(code.as_bytes()))
}

/// Entries by `(element, component)`.
pub(super) type Entries = BTreeMap<(usize, Option<usize>), ElementEntry>;

/// The occurrence's element entries by `(element, component)`; none may list
/// codes at the qualifier's place.
pub(super) fn compile_entries(
    segments: &BTreeMap<Vec<u8>, SegmentDef>,
    segment: &str,
    raw: &BTreeMap<ElementPosition, ElementEntryModel>,
    qualifier: Option<&Qualifier>,
) -> Result<Entries, OccurrenceError> {
    let mut entries = BTreeMap::new();
    for (&at, entry) in raw {
        let compiled = compile_entry(segments, segment, at, entry, qualifier)?;
        entries.insert((at.element, at.component), compiled);
    }
    Ok(entries)
}

/// One entry at `at`: the codes it lists, narrowing the element's.
fn compile_entry(
    segments: &BTreeMap<Vec<u8>, SegmentDef>,
    segment: &str,
    at: ElementPosition,
    entry: &ElementEntryModel,
    qualifier: Option<&Qualifier>,
) -> Result<ElementEntry, OccurrenceError> {
    let key = format!("elements.{at}");
    refuse_qualifier_codes(&key, at, entry, qualifier)?;
    let def = find_element(segments, segment, &key, at)?;
    Ok(ElementEntry {
        required: Setting::Element,
        length: Setting::Element,
        codes: entry_codes(def, segment, at, &key, entry)?,
    })
}

/// Refuses codes at the qualifier's own place: they are the qualifier's.
fn refuse_qualifier_codes(
    key: &str,
    at: ElementPosition,
    entry: &ElementEntryModel,
    qualifier: Option<&Qualifier>,
) -> Result<(), OccurrenceError> {
    let at_qualifier =
        qualifier.is_some_and(|q| (q.element, q.component) == (at.element, at.component));
    if at_qualifier && entry.codes.is_some() {
        return Err(OccurrenceError::QualifierInCodes {
            key: format!("{key}.codes"),
        });
    }
    Ok(())
}

/// The codes an entry lists, validated against its element; the element's
/// when it lists none.
fn entry_codes(
    def: &ElementDef,
    segment: &str,
    at: ElementPosition,
    key: &str,
    entry: &ElementEntryModel,
) -> Result<Setting<Vec<String>>, OccurrenceError> {
    let Some(codes) = &entry.codes else {
        return Ok(Setting::Element);
    };
    let list = CodeList {
        list: format!("{key}.codes"),
        element: at.element,
        component: at.component,
        codes,
    };
    compile_list(def, segment, list, def.length).map(Setting::Occurrence)
}
