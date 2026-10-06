//! Segment and element definitions, and the compilation of a segment's elements.

use std::collections::BTreeMap;
use std::fmt;

use super::error::SpecError;
use super::raw::RawElement;

/// The data type of an element, as the X12 standard names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementType {
    /// `AN`: a string.
    An,
    /// `ID`: a code from a list.
    Id,
    /// `N0` to `N9`: an integer with that many implied decimal places.
    N(u8),
    /// `R`: a decimal number with an explicit point, kept at `scale` places.
    R {
        /// Decimal places; 2 unless the spec says otherwise.
        scale: u8,
    },
    /// `DT`: a date, `CCYYMMDD` or `YYMMDD`.
    Dt,
    /// `TM`: a time, `HHMM` optionally followed by seconds and decimal seconds.
    Tm,
}

impl ElementType {
    /// Largest `scale` an `R` element may declare. An `R` value is held as an
    /// `i128` scaled by `10^scale` in a column of precision 38, so a scale of
    /// 18 still leaves 20 digits for the integer part.
    pub const MAX_SCALE: u8 = 18;

    /// Reads the type code of an element definition; `scale` is the
    /// definition's `scale` key, which only `R` accepts.
    pub(super) fn parse(code: &str, scale: Option<u8>) -> Result<ElementType, ElementDefError> {
        let kind = match code.as_bytes() {
            b"AN" => ElementType::An,
            b"ID" => ElementType::Id,
            b"R" => ElementType::R {
                scale: scale.unwrap_or(2),
            },
            b"DT" => ElementType::Dt,
            b"TM" => ElementType::Tm,
            [b'N', digit @ b'0'..=b'9'] => ElementType::N(digit - b'0'),
            _ => {
                return Err(ElementDefError::UnknownType {
                    found: code.to_string(),
                });
            }
        };
        if scale.is_some() && !matches!(kind, ElementType::R { .. }) {
            return Err(ElementDefError::ScaleWithoutR {
                kind: code.to_string(),
            });
        }
        Ok(kind)
    }

    /// The length of a value as X12 counts it: numeric types count digits
    /// only (no sign, no point), every other type counts bytes.
    pub(crate) fn length_of(self, text: &[u8]) -> usize {
        match self {
            ElementType::N(_) | ElementType::R { .. } => {
                text.iter().filter(|byte| byte.is_ascii_digit()).count()
            }
            _ => text.len(),
        }
    }
}

impl fmt::Display for ElementType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ElementType::An => write!(f, "AN (string)"),
            ElementType::Id => write!(f, "ID (code)"),
            ElementType::N(places) => {
                write!(f, "N{places} (integer with {places} implied decimals)")
            }
            ElementType::R { scale } => write!(f, "R (decimal, scale {scale})"),
            ElementType::Dt => write!(f, "DT (date CCYYMMDD or YYMMDD)"),
            ElementType::Tm => write!(f, "TM (time HHMM, HHMMSS or HHMMSSD..)"),
        }
    }
}

/// One element of a segment, or one component of a composite element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementDef {
    /// Name used for the element downstream, e.g. `claim_submitter_id`.
    pub name: String,
    /// Data type.
    pub kind: ElementType,
    /// `true` when the element must be present and non-empty.
    pub required: bool,
    /// Minimum length, when the spec sets one.
    pub min: Option<usize>,
    /// Maximum length, when the spec sets one.
    pub max: Option<usize>,
    /// Components by 1-based position; empty for a simple element.
    pub composite: BTreeMap<usize, ElementDef>,
    /// The values the element may hold, sorted by their bytes; empty when
    /// any value of its type is accepted.
    pub codes: Vec<String>,
}

impl ElementDef {
    /// `true` when the element has a code list and `value` is not in it.
    pub(crate) fn rejects_code(&self, value: &[u8]) -> bool {
        rejects_code(&self.codes, value)
    }
}

/// `true` when `codes` (sorted by bytes) is a code list and `value` is not
/// in it; an empty list rejects nothing.
pub(crate) fn rejects_code(codes: &[String], value: &[u8]) -> bool {
    !codes.is_empty()
        && codes
            .binary_search_by(|code| code.as_bytes().cmp(value))
            .is_err()
}

/// The elements of one segment id.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SegmentDef {
    /// Elements by 1-based position. Positions with no entry are opaque.
    pub elements: BTreeMap<usize, ElementDef>,
}

/// Why an element definition was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElementDefError {
    /// The position key is not a 1-based integer in canonical form.
    NonCanonicalPosition,
    /// `name` is the empty string.
    EmptyName,
    /// Another element at the same level already has this name.
    DuplicateName {
        /// The repeated name.
        name: String,
        /// Position key of the element that used it first, as written.
        first: String,
    },
    /// `type` is not one of the known codes.
    UnknownType {
        /// The code as written.
        found: String,
    },
    /// `scale` was given for a type other than `R`.
    ScaleWithoutR {
        /// The type code as written.
        kind: String,
    },
    /// `min` is greater than `max`.
    MinAboveMax {
        /// The minimum as written.
        min: usize,
        /// The maximum as written.
        max: usize,
    },
    /// `composite` was given for a type other than `AN`.
    CompositeOnNonAn {
        /// The type code as written.
        kind: String,
    },
    /// A component declares a `composite` of its own.
    NestedComposite,
    /// `scale` is above [`ElementType::MAX_SCALE`].
    ScaleAboveMaximum {
        /// The scale as written.
        scale: u8,
    },
    /// `max` is 0, so no value could ever be valid.
    ZeroMax,
    /// `codes` is an empty list.
    EmptyCodes,
    /// A code is the empty string.
    EmptyCode {
        /// The code's 0-based index in `codes` as written.
        index: usize,
    },
    /// A code's length is outside the element's `min` and `max`.
    CodeLength {
        /// The code's 0-based index in `codes` as written.
        index: usize,
        /// The code as written.
        code: String,
        /// Its length, counted as the element check counts it.
        length: usize,
        /// The element's minimum length.
        min: Option<usize>,
        /// The element's maximum length.
        max: Option<usize>,
    },
    /// A code is listed more than once.
    DuplicateCode {
        /// The code as written.
        code: String,
        /// 0-based index of its first listing.
        first: usize,
        /// 0-based index of its repeat.
        second: usize,
    },
    /// `codes` was given on an element that declares a `composite`, whose
    /// value is checked component by component.
    CodesOnComposite,
    /// `codes` was given on a numeric type (`N0` to `N9`, `R`), whose values
    /// are numbers: `1`, `01` and `1.0` are one value written three ways, so
    /// a list compared byte for byte would reject some of them.
    CodesOnNumeric {
        /// The type code as written.
        kind: String,
    },
}

impl fmt::Display for ElementDefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ElementDefError::NonCanonicalPosition => write!(
                f,
                "positions are 1-based integers written in canonical form"
            ),
            ElementDefError::EmptyName => write!(f, "\"name\" is empty"),
            ElementDefError::DuplicateName { name, first } => {
                write!(f, "name {name:?} is already used by position {first:?}")
            }
            ElementDefError::UnknownType { found } => write!(
                f,
                "type {found:?} is not one of AN, ID, N0 to N9, R, DT, TM"
            ),
            ElementDefError::ScaleWithoutR { kind } => {
                write!(f, "\"scale\" applies only to type R; found type {kind:?}")
            }
            ElementDefError::MinAboveMax { min, max } => {
                write!(f, "\"min\" {min} is greater than \"max\" {max}")
            }
            ElementDefError::CompositeOnNonAn { kind } => {
                write!(f, "\"composite\" requires type AN; found type {kind:?}")
            }
            ElementDefError::NestedComposite => {
                write!(f, "a component cannot declare its own \"composite\"")
            }
            ElementDefError::ScaleAboveMaximum { scale } => write!(
                f,
                "\"scale\" {scale} is above the maximum of {}",
                ElementType::MAX_SCALE
            ),
            ElementDefError::ZeroMax => {
                write!(f, "\"max\" is 0; an element holds at least one character")
            }
            ElementDefError::EmptyCodes => write!(
                f,
                "\"codes\" is empty; leave the key out to accept any value"
            ),
            ElementDefError::EmptyCode { index } => {
                write!(f, "the code at codes[{index}] is empty")
            }
            ElementDefError::CodeLength {
                index,
                code,
                length,
                min,
                max,
            } => {
                write!(
                    f,
                    "code {code:?} at codes[{index}] has length {length}; the element allows "
                )?;
                match (min, max) {
                    (Some(min), Some(max)) => write!(f, "{min} to {max}"),
                    (Some(min), None) => write!(f, "at least {min}"),
                    (None, Some(max)) => write!(f, "at most {max}"),
                    (None, None) => write!(f, "any length"),
                }
            }
            ElementDefError::DuplicateCode {
                code,
                first,
                second,
            } => write!(
                f,
                "code {code:?} is listed twice, at codes[{first}] and codes[{second}]"
            ),
            ElementDefError::CodesOnComposite => write!(
                f,
                "\"codes\" applies to a simple element or a component; this element declares a \"composite\""
            ),
            ElementDefError::CodesOnNumeric { kind } => write!(
                f,
                "\"codes\" applies to non-numeric types; type {kind:?} holds numbers, which one value can write several ways (1, 01, 1.0)"
            ),
        }
    }
}

/// Name of the automatic column that numbers a table's rows from 0, across
/// the whole stream.
pub const ROW_COLUMN: &str = "row";

/// Name of the automatic column that holds the index of a row's anchor
/// segment: the segment that opened the anchor loop instance, or the
/// anchored segment itself.
pub const SEGMENT_COLUMN: &str = "segment";

/// A 1-based element position written in canonical form (`"1"`, never
/// `"01"` or `"+1"`), so no two keys can name the same position.
pub(super) fn parse_position(key: &str) -> Option<usize> {
    key.parse::<usize>()
        .ok()
        .filter(|&p| p >= 1 && p.to_string() == key)
}

/// Compiles the elements of `segment`, or the components of the element at
/// `parent` (its key as written) when one is given.
pub(super) fn compile_elements(
    segment: &str,
    raw: &BTreeMap<String, RawElement>,
    parent: Option<&str>,
) -> Result<BTreeMap<usize, ElementDef>, SpecError> {
    let mut elements = BTreeMap::new();
    let mut names: BTreeMap<&str, &str> = BTreeMap::new();
    for (key, def) in raw {
        let position_text = match parent {
            Some(parent) => format!("{parent}.composite.{key}"),
            None => key.clone(),
        };
        let fail = |reason| SpecError::BadElementDef {
            segment: segment.to_string(),
            position: position_text.clone(),
            reason,
        };
        let position =
            parse_position(key).ok_or_else(|| fail(ElementDefError::NonCanonicalPosition))?;
        if def.name.is_empty() {
            return Err(fail(ElementDefError::EmptyName));
        }
        if let Some(first) = names.insert(def.name.as_str(), key.as_str()) {
            return Err(fail(ElementDefError::DuplicateName {
                name: def.name.clone(),
                first: first.to_string(),
            }));
        }
        let kind = ElementType::parse(&def.kind, def.scale).map_err(fail)?;
        if let ElementType::R { scale } = kind
            && scale > ElementType::MAX_SCALE
        {
            return Err(fail(ElementDefError::ScaleAboveMaximum { scale }));
        }
        if def.max == Some(0) {
            return Err(fail(ElementDefError::ZeroMax));
        }
        if let (Some(min), Some(max)) = (def.min, def.max)
            && min > max
        {
            return Err(fail(ElementDefError::MinAboveMax { min, max }));
        }
        if def.codes.is_some() && !def.composite.is_empty() {
            return Err(fail(ElementDefError::CodesOnComposite));
        }
        if def.codes.is_some() && matches!(kind, ElementType::N(_) | ElementType::R { .. }) {
            return Err(fail(ElementDefError::CodesOnNumeric {
                kind: def.kind.clone(),
            }));
        }
        let codes = match &def.codes {
            None => Vec::new(),
            Some(codes) => compile_codes(codes, kind, def.min, def.max).map_err(fail)?,
        };
        let composite = if def.composite.is_empty() {
            BTreeMap::new()
        } else if parent.is_some() {
            return Err(fail(ElementDefError::NestedComposite));
        } else if kind != ElementType::An {
            return Err(fail(ElementDefError::CompositeOnNonAn {
                kind: def.kind.clone(),
            }));
        } else {
            compile_elements(segment, &def.composite, Some(key))?
        };
        elements.insert(
            position,
            ElementDef {
                name: def.name.clone(),
                kind,
                required: def.required,
                min: def.min,
                max: def.max,
                composite,
                codes,
            },
        );
    }
    Ok(elements)
}

/// Validates a code list against its element's type and lengths and returns
/// it sorted by bytes, the order the element check searches.
pub(super) fn compile_codes(
    codes: &[String],
    kind: ElementType,
    min: Option<usize>,
    max: Option<usize>,
) -> Result<Vec<String>, ElementDefError> {
    if codes.is_empty() {
        return Err(ElementDefError::EmptyCodes);
    }
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for (index, code) in codes.iter().enumerate() {
        if code.is_empty() {
            return Err(ElementDefError::EmptyCode { index });
        }
        let length = kind.length_of(code.as_bytes());
        if min.is_some_and(|min| length < min) || max.is_some_and(|max| length > max) {
            return Err(ElementDefError::CodeLength {
                index,
                code: code.clone(),
                length,
                min,
                max,
            });
        }
        if let Some(first) = seen.insert(code.as_str(), index) {
            return Err(ElementDefError::DuplicateCode {
                code: code.clone(),
                first,
                second: index,
            });
        }
    }
    Ok(seen.into_keys().map(str::to_string).collect())
}
