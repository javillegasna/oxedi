//! Element checks: the values a segment's elements parse to, and the diagnostics they raise.

use std::borrow::Cow;

use crate::diagnostic::Rule;
use crate::element::Element;
use crate::segment::Segment;
use crate::spec::ElementRule;
use crate::value::{ColumnType, Parsed};

use super::Projector;
use super::fill::leaf_text;
use super::plan::{ElementCheck, LeafCheck};

/// One defined element (or component) of the current segment, checked.
#[derive(Debug, Clone, Copy)]
pub(super) struct Checked {
    pub(super) element: usize,
    pub(super) component: Option<usize>,
    /// The column type its definition maps to.
    pub(super) kind: ColumnType,
    /// The parsed value; `None` when missing or of the wrong type.
    pub(super) value: Option<Parsed>,
}

impl<'s> Projector<'s> {
    /// Checks every element the spec defines for the segment and keeps the
    /// parsed values of the ones a column reads.
    pub(super) fn check(
        &mut self,
        elements: &[ElementCheck<'_>],
        segment: &Segment<'_>,
        joined: &mut Vec<u8>,
    ) {
        self.checked.clear();
        for check in elements {
            if check
                .composite
                .is_some_and(|declared| !self.composite_holds(check, declared, segment))
            {
                continue;
            }
            for leaf in &check.leaves {
                self.check_leaf(check.position, leaf, segment, joined);
            }
        }
    }

    /// Checks a composite as a whole: an empty one is reported when required
    /// and its components are not checked (`false`); more components than
    /// declared are reported.
    fn composite_holds(
        &mut self,
        check: &ElementCheck<'_>,
        declared: usize,
        segment: &Segment<'_>,
    ) -> bool {
        let position = check.position;
        let parts = composite_parts(segment, position);
        if parts.iter().all(|part| part.is_empty()) {
            let rule = ElementRule::at(check.def, self.occurrence, position, None);
            if rule.required {
                self.report(
                    Rule::RequiredElementMissing {
                        segment_id: segment.id.to_vec(),
                        element: position,
                        component: None,
                        name: check.def.name.clone(),
                        occurrence: rule.required_of.map(str::to_owned),
                    },
                    segment.index,
                    position,
                    None,
                    &[],
                );
            }
            return false;
        }
        if parts.len() > declared {
            let extra = parts.get(declared).map_or(&[][..], |part| part.as_ref());
            self.report(
                Rule::CompositeShape {
                    segment_id: segment.id.to_vec(),
                    element: position,
                    name: check.def.name.clone(),
                    declared,
                    found: parts.len(),
                },
                segment.index,
                position,
                declared.checked_add(1),
                extra,
            );
        }
        true
    }

    /// Checks one leaf and keeps its parsed value when a column reads it.
    fn check_leaf(
        &mut self,
        position: usize,
        leaf: &LeafCheck<'_>,
        segment: &Segment<'_>,
        joined: &mut Vec<u8>,
    ) {
        let text = leaf_text(segment, position, leaf.component, self.separator, joined);
        let rule = ElementRule::at(leaf.def, self.occurrence, position, leaf.component);
        let value = self.check_value(segment, position, leaf.component, rule, text, leaf.read);
        if leaf.read {
            self.checked.push(Checked {
                element: position,
                component: leaf.component,
                kind: leaf.kind,
                value,
            });
        }
    }

    /// Checks one value against its definition, reports what fails, and
    /// returns the parsed value (`None` when missing or of the wrong type).
    /// When no column reads it (`keep` false) the value is only validated
    /// and `None` is returned; the diagnostics are the same.
    fn check_value(
        &mut self,
        segment: &Segment<'_>,
        element: usize,
        component: Option<usize>,
        rule: ElementRule<'_>,
        text: &[u8],
        keep: bool,
    ) -> Option<Parsed> {
        let def = rule.def;
        if text.is_empty() {
            if rule.required {
                self.report(
                    Rule::RequiredElementMissing {
                        segment_id: segment.id.to_vec(),
                        element,
                        component,
                        name: def.name.clone(),
                        occurrence: rule.required_of.map(str::to_owned),
                    },
                    segment.index,
                    element,
                    component,
                    &[],
                );
            }
            return None;
        }
        let Some(value) = ColumnType::of(Some(def.kind)).parse(text) else {
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
            return None;
        };
        let value = keep.then_some(value);
        // Numeric lengths count digits only, as X12 does: no sign, no point.
        if !def.kind.length_in_range(text, rule.length) {
            self.report(
                Rule::LengthOutOfRange {
                    segment_id: segment.id.to_vec(),
                    element,
                    component,
                    name: def.name.clone(),
                    allowed: rule.length,
                    length: def.kind.length_of(text),
                    occurrence: rule.length_of.map(str::to_owned),
                },
                segment.index,
                element,
                component,
                text,
            );
        } else {
            // Lengths are checked first, so a value of the wrong length is
            // reported once, as a length.
            if !rule.rejects(text) {
                return value;
            }
            self.report(
                Rule::CodeNotInList {
                    segment_id: segment.id.to_vec(),
                    element,
                    component,
                    name: def.name.clone(),
                    codes: rule.codes.to_vec(),
                    occurrence: rule.codes_of.map(str::to_owned),
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

/// The components of the element at `position`; a simple element is its own
/// only component, a missing one has none.
fn composite_parts<'a>(segment: &'a Segment<'_>, position: usize) -> &'a [Cow<'a, [u8]>] {
    match segment.element(position) {
        Some(Element::Composite(parts)) => parts,
        Some(Element::Simple(value)) => std::slice::from_ref(value),
        None => &[],
    }
}
