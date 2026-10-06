//! Segment occurrences: the named places a segment takes inside a loop, with
//! their usage, maximum repeat, position, qualifier and own code lists, and
//! their compilation against the `segments` section.

use std::collections::BTreeMap;
use std::fmt;

use super::error::SpecError;
use super::loops::Trigger;
use super::raw::{RawOccurrence, RawQualifier};
use super::render::render_key;
use super::segments::{
    ElementDef, ElementDefError, ElementType, SegmentDef, compile_codes, parse_position,
};
use crate::element::Element;
use crate::segment::Segment;

/// Whether an occurrence must appear in every instance of its loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Usage {
    /// Written `required`: every instance of the loop holds it.
    Required,
    /// Written `situational`: an instance may hold it or not.
    Situational,
}

/// The element (or component) and the codes that tell an occurrence apart
/// from the other occurrences of its segment in the same loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Qualifier {
    /// 1-based element position.
    pub element: usize,
    /// 1-based component position, when the qualifier is a component.
    pub component: Option<usize>,
    /// The codes that identify the occurrence, sorted by their bytes.
    pub codes: Vec<String>,
}

impl Qualifier {
    /// `true` when the segment holds one of the codes at the qualifier's place.
    /// A simple element counts as its own first component.
    pub fn matches(&self, segment: &Segment<'_>) -> bool {
        let value = match (segment.element(self.element), self.component) {
            (Some(Element::Simple(value)), None | Some(1)) => Some(value.as_ref()),
            (Some(Element::Composite(parts)), Some(component)) => component
                .checked_sub(1)
                .and_then(|index| parts.get(index))
                .map(AsRef::as_ref),
            _ => None,
        };
        value.is_some_and(|value| {
            self.codes
                .binary_search_by(|code| code.as_bytes().cmp(value))
                .is_ok()
        })
    }
}

/// One named place a segment takes inside a loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OccurrenceDef {
    /// Name used in the JSON, unique within the loop, e.g. `patient_name`.
    pub name: String,
    /// Segment id, e.g. `NM1`.
    pub segment: Vec<u8>,
    /// Position in the loop; occurrences sharing a position may appear in
    /// any order among themselves.
    pub pos: usize,
    /// Whether every instance of the loop holds it.
    pub usage: Usage,
    /// Most times it may repeat in one instance of the loop; `None` for no limit.
    pub max: Option<usize>,
    /// What tells it apart from the other occurrences of its segment.
    pub qualifier: Option<Qualifier>,
    /// Code lists of its own, by `(element, component)`, each sorted by bytes.
    pub codes: BTreeMap<(usize, Option<usize>), Vec<String>>,
}

impl OccurrenceDef {
    /// `true` when the segment has this occurrence's id and, when it has a
    /// qualifier, one of its codes.
    pub fn matches(&self, segment: &Segment<'_>) -> bool {
        segment.id == self.segment.as_slice()
            && self
                .qualifier
                .as_ref()
                .is_none_or(|qualifier| qualifier.matches(segment))
    }

    /// `true` when the trigger opens the loop on this occurrence: same
    /// segment and, when the occurrence has a qualifier on an element, a
    /// trigger condition on that element with one of its codes.
    pub(super) fn opens_on(&self, trigger: &Trigger) -> bool {
        if self.segment != trigger.segment {
            return false;
        }
        let Some(qualifier) = &self.qualifier else {
            return true;
        };
        qualifier.component.is_none()
            && trigger.conditions.iter().any(|(position, value)| {
                *position == qualifier.element
                    && qualifier.codes.iter().any(|code| code.as_bytes() == value)
            })
    }
}

/// Why an occurrence was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OccurrenceError {
    /// The occurrence name is the empty string.
    EmptyName,
    /// `usage` is not `required` or `situational`.
    UnknownUsage {
        /// The value as written.
        found: String,
    },
    /// `max` is 0.
    ZeroMax,
    /// The qualifier or a `codes` key names an element or component the
    /// `segments` section does not define for the segment.
    UndefinedElement {
        /// Where the reference sits: `qualifier` or `codes.<key>`.
        key: String,
        /// The segment id.
        segment: String,
        /// The element position.
        element: usize,
        /// The component position, if any.
        component: Option<usize>,
    },
    /// A `codes` key is not `<element>` or `<element>-<component>`, 1-based
    /// in canonical form.
    BadCodesKey {
        /// The key as written.
        key: String,
    },
    /// A code list (the qualifier's or one under `codes`) is empty.
    EmptyCodes {
        /// Where the list sits: `qualifier.codes` or `codes.<key>`.
        key: String,
    },
    /// A code list (the qualifier's or one under `codes`) is invalid for its element.
    BadCodes {
        /// Where the list sits: `qualifier.codes` or `codes.<key>`.
        key: String,
        /// What is wrong with it.
        reason: ElementDefError,
    },
    /// A `codes` key names the qualifier's own element or component.
    QualifierInCodes {
        /// The key as written.
        key: String,
    },
    /// Another occurrence holds the same segment and the two have no
    /// qualifier on one shared element or component to tell them apart.
    Indistinct {
        /// The other occurrence.
        other: String,
        /// The segment id both hold.
        segment: String,
    },
    /// Another occurrence holds the same segment and both qualifiers accept a code.
    SharedCode {
        /// The other occurrence.
        other: String,
        /// The segment id both hold.
        segment: String,
        /// The code both accept.
        code: String,
    },
}

fn place(segment: &str, element: usize, component: Option<usize>) -> String {
    match component {
        Some(component) => format!("{segment}{element:02}-{component}"),
        None => format!("{segment}{element:02}"),
    }
}

impl fmt::Display for OccurrenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OccurrenceError::EmptyName => write!(f, "the occurrence name is empty"),
            OccurrenceError::UnknownUsage { found } => write!(
                f,
                "\"usage\" must be \"required\" or \"situational\"; found {found:?}"
            ),
            OccurrenceError::ZeroMax => write!(
                f,
                "\"max\" is 0; an occurrence that may appear appears at least once"
            ),
            OccurrenceError::UndefinedElement {
                key,
                segment,
                element,
                component,
            } => write!(
                f,
                "{key} names {}, which the \"segments\" section does not define for {segment:?}",
                place(segment, *element, *component)
            ),
            OccurrenceError::BadCodesKey { key } => write!(
                f,
                "codes key {key:?} must be \"<element>\" or \"<element>-<component>\", \
                 1-based integers in canonical form"
            ),
            OccurrenceError::EmptyCodes { key } => {
                write!(f, "{key} is empty; list at least one code")
            }
            OccurrenceError::BadCodes { key, reason } => write!(f, "{key}: {reason}"),
            OccurrenceError::QualifierInCodes { key } => write!(
                f,
                "codes key {key:?} names the qualifier's own place; its codes are the \
                 qualifier's \"codes\""
            ),
            OccurrenceError::Indistinct { other, segment } => write!(
                f,
                "holds segment {segment:?} like occurrence {other:?}, and the two have no \
                 qualifier on one shared element to tell them apart"
            ),
            OccurrenceError::SharedCode {
                other,
                segment,
                code,
            } => write!(
                f,
                "holds segment {segment:?} like occurrence {other:?}, and both qualifiers \
                 accept code {code:?}"
            ),
        }
    }
}

/// The type code of an element type, as a spec writes it.
fn type_code(kind: ElementType) -> String {
    match kind {
        ElementType::An => "AN".into(),
        ElementType::Id => "ID".into(),
        ElementType::N(places) => format!("N{places}"),
        ElementType::R { .. } => "R".into(),
        ElementType::Dt => "DT".into(),
        ElementType::Tm => "TM".into(),
    }
}

/// The definition of an element or component of `segment`, if any.
fn element_def<'d>(
    segments: &'d BTreeMap<Vec<u8>, SegmentDef>,
    segment: &[u8],
    element: usize,
    component: Option<usize>,
) -> Option<&'d ElementDef> {
    let def = segments.get(segment)?.elements.get(&element)?;
    match component {
        None => Some(def),
        Some(component) => def.composite.get(&component),
    }
}

/// Validates a code list against the element it is for, with the checks an
/// element's own `codes` gets, and returns it sorted by bytes.
fn element_codes(def: &ElementDef, codes: &[String]) -> Result<Vec<String>, ElementDefError> {
    if !def.composite.is_empty() {
        return Err(ElementDefError::CodesOnComposite);
    }
    if matches!(def.kind, ElementType::N(_) | ElementType::R { .. }) {
        return Err(ElementDefError::CodesOnNumeric {
            kind: type_code(def.kind),
        });
    }
    compile_codes(codes, def.kind, def.min, def.max)
}

/// Reads a `codes` key: `<element>` or `<element>-<component>`.
fn parse_codes_key(key: &str) -> Option<(usize, Option<usize>)> {
    match key.split_once('-') {
        None => Some((parse_position(key)?, None)),
        Some((element, component)) => {
            Some((parse_position(element)?, Some(parse_position(component)?)))
        }
    }
}

/// Compiles the occurrences of the loop `loop_name` against the segment
/// definitions, sorted by position (ties keep name order), and checks that
/// every two occurrences of one segment can be told apart.
pub(super) fn compile_occurrences(
    loop_name: &str,
    raw: &BTreeMap<String, RawOccurrence>,
    segments: &BTreeMap<Vec<u8>, SegmentDef>,
) -> Result<Vec<OccurrenceDef>, SpecError> {
    let mut occurrences = Vec::with_capacity(raw.len());
    for (name, def) in raw {
        occurrences.push(compile_occurrence(loop_name, name, def, segments)?);
    }
    occurrences.sort_by_key(|occurrence| occurrence.pos);
    for (i, first) in occurrences.iter().enumerate() {
        for second in occurrences.iter().skip(i + 1) {
            if first.segment != second.segment {
                continue;
            }
            let fail = |reason| SpecError::BadOccurrence {
                loop_name: loop_name.to_string(),
                occurrence: second.name.clone(),
                reason: Box::new(reason),
            };
            let segment = String::from_utf8_lossy(&second.segment).into_owned();
            let (Some(a), Some(b)) = (&first.qualifier, &second.qualifier) else {
                return Err(fail(OccurrenceError::Indistinct {
                    other: first.name.clone(),
                    segment,
                }));
            };
            if (a.element, a.component) != (b.element, b.component) {
                return Err(fail(OccurrenceError::Indistinct {
                    other: first.name.clone(),
                    segment,
                }));
            }
            if let Some(code) = a.codes.iter().find(|code| b.codes.contains(code)) {
                return Err(fail(OccurrenceError::SharedCode {
                    other: first.name.clone(),
                    segment,
                    code: code.clone(),
                }));
            }
        }
    }
    Ok(occurrences)
}

fn compile_occurrence(
    loop_name: &str,
    name: &str,
    def: &RawOccurrence,
    segments: &BTreeMap<Vec<u8>, SegmentDef>,
) -> Result<OccurrenceDef, SpecError> {
    let fail = |reason| SpecError::BadOccurrence {
        loop_name: loop_name.to_string(),
        occurrence: name.to_string(),
        reason: Box::new(reason),
    };
    if name.is_empty() {
        return Err(fail(OccurrenceError::EmptyName));
    }
    if def.segment.is_empty() {
        return Err(SpecError::EmptySegmentId {
            loop_name: Some(loop_name.to_string()),
            key: format!("occurrences.{}.segment", render_key(name)),
        });
    }
    let segment = def.segment.as_bytes();
    let usage = match def.usage.as_deref() {
        None | Some("situational") => Usage::Situational,
        Some("required") => Usage::Required,
        Some(found) => {
            return Err(fail(OccurrenceError::UnknownUsage {
                found: found.to_string(),
            }));
        }
    };
    if def.max == Some(0) {
        return Err(fail(OccurrenceError::ZeroMax));
    }
    let undefined = |key: String, element, component| OccurrenceError::UndefinedElement {
        key,
        segment: def.segment.clone(),
        element,
        component,
    };
    let qualifier = match &def.qualifier {
        None => None,
        Some(RawQualifier {
            element,
            component,
            codes,
        }) => {
            let target = element_def(segments, segment, *element, *component)
                .ok_or_else(|| fail(undefined("qualifier".into(), *element, *component)))?;
            if codes.is_empty() {
                return Err(fail(OccurrenceError::EmptyCodes {
                    key: "qualifier.codes".into(),
                }));
            }
            let codes = element_codes(target, codes).map_err(|reason| {
                fail(OccurrenceError::BadCodes {
                    key: "qualifier.codes".into(),
                    reason,
                })
            })?;
            Some(Qualifier {
                element: *element,
                component: *component,
                codes,
            })
        }
    };
    let mut codes = BTreeMap::new();
    for (key, list) in &def.codes {
        let (element, component) = parse_codes_key(key)
            .ok_or_else(|| fail(OccurrenceError::BadCodesKey { key: key.clone() }))?;
        if qualifier
            .as_ref()
            .is_some_and(|q| (q.element, q.component) == (element, component))
        {
            return Err(fail(OccurrenceError::QualifierInCodes { key: key.clone() }));
        }
        let at = format!("codes.{key}");
        let target = element_def(segments, segment, element, component)
            .ok_or_else(|| fail(undefined(at.clone(), element, component)))?;
        if list.is_empty() {
            return Err(fail(OccurrenceError::EmptyCodes { key: at }));
        }
        let list = element_codes(target, list)
            .map_err(|reason| fail(OccurrenceError::BadCodes { key: at, reason }))?;
        codes.insert((element, component), list);
    }
    Ok(OccurrenceDef {
        name: name.to_string(),
        segment: segment.to_vec(),
        pos: def.pos,
        usage,
        max: def.max,
        qualifier,
        codes,
    })
}
