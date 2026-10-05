//! Element checks: the values a segment's elements parse to, and the diagnostics they raise.

use crate::column::{ColumnType, is_dt, parse_dt, parse_n, parse_r, parse_tm};
use crate::diagnostic::Rule;
use crate::element::Element;
use crate::segment::Segment;
use crate::spec::ElementDef;

use super::Projector;
use super::fill::leaf_text;
use super::plan::ElementPlan;
/// A value an element check parsed, or a column read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Parsed {
    /// Absent, empty, or not a valid value of its type.
    Null,
    /// Text, to be copied from the segment.
    Text,
    Int(i64),
    Decimal(i128),
    Date(i32),
    Time(i32),
}

/// One defined element (or component) of the current segment, checked.
#[derive(Debug, Clone, Copy)]
pub(super) struct Checked {
    pub(super) element: usize,
    pub(super) component: Option<usize>,
    /// The column type its definition maps to.
    pub(super) kind: ColumnType,
    pub(super) value: Parsed,
}

/// Whether a non-empty text is a value of the column type, as [`parse`]
/// decides, without keeping the value: text is always valid and a date skips
/// the day count.
fn is_valid(kind: ColumnType, text: &[u8]) -> bool {
    match kind {
        ColumnType::Binary => true,
        ColumnType::Int64 { .. } => parse_n(text).is_some(),
        ColumnType::Decimal128 { scale, .. } => parse_r(text, scale).is_some(),
        ColumnType::Date32 => is_dt(text),
        ColumnType::Time32 => parse_tm(text).is_some(),
    }
}

/// A non-empty text as a value of the column type; `None` when it does not parse.
pub(super) fn parse(kind: ColumnType, text: &[u8]) -> Option<Parsed> {
    Some(match kind {
        ColumnType::Binary => Parsed::Text,
        ColumnType::Int64 { .. } => Parsed::Int(parse_n(text)?),
        ColumnType::Decimal128 { scale, .. } => Parsed::Decimal(parse_r(text, scale)?),
        ColumnType::Date32 => Parsed::Date(parse_dt(text)?),
        ColumnType::Time32 => Parsed::Time(parse_tm(text)?),
    })
}

impl<'s> Projector<'s> {
    /// Checks every element the spec defines for the segment and keeps the
    /// parsed values of the ones a column reads.
    pub(super) fn check(
        &mut self,
        elements: &[ElementPlan<'_>],
        segment: &Segment<'_>,
        joined: &mut Vec<u8>,
    ) {
        self.checked.clear();
        for plan in elements {
            let (position, element) = (plan.position, plan.def);
            if plan.components.is_empty() {
                let text = leaf_text(segment, position, None, self.separator, joined);
                let value = self.check_value(segment, position, None, element, text, plan.read);
                if plan.read {
                    self.checked.push(Checked {
                        element: position,
                        component: None,
                        kind: plan.kind,
                        value,
                    });
                }
                continue;
            }
            let parts: &[std::borrow::Cow<'_, [u8]>] = match segment.element(position) {
                Some(Element::Composite(parts)) => parts,
                Some(Element::Simple(value)) => std::slice::from_ref(value),
                None => &[],
            };
            if parts.iter().all(|part| part.is_empty()) {
                if element.required {
                    self.report(
                        Rule::RequiredElementMissing {
                            segment_id: segment.id.to_vec(),
                            element: position,
                            component: None,
                            name: element.name.clone(),
                        },
                        segment.index,
                        position,
                        None,
                        &[],
                    );
                }
                continue;
            }
            let declared = plan.declared;
            if parts.len() > declared {
                let extra = parts.get(declared).map_or(&[][..], |part| part.as_ref());
                self.report(
                    Rule::CompositeShape {
                        segment_id: segment.id.to_vec(),
                        element: position,
                        name: element.name.clone(),
                        declared,
                        found: parts.len(),
                    },
                    segment.index,
                    position,
                    declared.checked_add(1),
                    extra,
                );
            }
            for &(component, def, kind, read) in &plan.components {
                let text = component
                    .checked_sub(1)
                    .and_then(|at| parts.get(at))
                    .map_or(&[][..], |part| part.as_ref());
                let value = self.check_value(segment, position, Some(component), def, text, read);
                if read {
                    self.checked.push(Checked {
                        element: position,
                        component: Some(component),
                        kind,
                        value,
                    });
                }
            }
        }
    }

    /// Checks one value against its definition, reports what fails, and
    /// returns the parsed value (null when missing or of the wrong type).
    /// When no column reads it (`keep` false) the value is only validated
    /// and null is returned; the diagnostics are the same.
    fn check_value(
        &mut self,
        segment: &Segment<'_>,
        element: usize,
        component: Option<usize>,
        def: &ElementDef,
        text: &[u8],
        keep: bool,
    ) -> Parsed {
        if text.is_empty() {
            if def.required {
                self.report(
                    Rule::RequiredElementMissing {
                        segment_id: segment.id.to_vec(),
                        element,
                        component,
                        name: def.name.clone(),
                    },
                    segment.index,
                    element,
                    component,
                    &[],
                );
            }
            return Parsed::Null;
        }
        let kind = ColumnType::of(Some(def.kind));
        let value = if keep {
            parse(kind, text)
        } else {
            is_valid(kind, text).then_some(Parsed::Null)
        };
        let Some(value) = value else {
            self.report(
                Rule::TypeMismatch {
                    segment_id: segment.id.to_vec(),
                    element,
                    component,
                    name: def.name.clone(),
                    expected: def.kind,
                },
                segment.index,
                element,
                component,
                text,
            );
            return Parsed::Null;
        };
        // Numeric lengths count digits only, as X12 does: no sign, no point.
        let length = def.kind.length_of(text);
        if def.min.is_some_and(|min| length < min) || def.max.is_some_and(|max| length > max) {
            self.report(
                Rule::LengthOutOfRange {
                    segment_id: segment.id.to_vec(),
                    element,
                    component,
                    name: def.name.clone(),
                    min: def.min,
                    max: def.max,
                    length,
                },
                segment.index,
                element,
                component,
                text,
            );
        } else if def.rejects_code(text) {
            // Every code fits the element's lengths, so a value of the wrong
            // length is reported once, as a length.
            self.report(
                Rule::CodeNotInList {
                    segment_id: segment.id.to_vec(),
                    element,
                    component,
                    name: def.name.clone(),
                    codes: def.codes.clone(),
                },
                segment.index,
                element,
                component,
                text,
            );
        }
        value
    }
}
