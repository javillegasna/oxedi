//! The one rule that says what an element or component allows where a segment is read or written.

use super::{ElementDef, OccurrenceDef, rejects_code};
use crate::value::LengthRange;

/// What an element or component allows where a segment is read or
/// written: its definition, narrowed by the entry of the occurrence the
/// segment takes, when that occurrence has one.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ElementRule<'s> {
    /// The definition (name and type).
    pub(crate) def: &'s ElementDef,
    /// Whether the place must hold a value.
    pub(crate) required: bool,
    /// The lengths it allows.
    pub(crate) length: LengthRange,
    /// The codes it allows, sorted by bytes; empty for any value of its type.
    pub(crate) codes: &'s [String],
    /// The occurrence whose own list `codes` is; `None` when it is the
    /// element's.
    pub(crate) codes_of: Option<&'s str>,
}

impl<'s> ElementRule<'s> {
    /// The element's own rule.
    pub(crate) fn of(def: &'s ElementDef) -> Self {
        Self {
            def,
            required: def.required,
            length: def.length,
            codes: def.codes.as_slice(),
            codes_of: None,
        }
    }

    /// The rule at `(element, component)` of `occurrence`, when given, for
    /// `def`. One map lookup over the places the occurrence narrows, and no
    /// allocation. `element` and `component` are 1-based.
    pub(crate) fn at(
        def: &'s ElementDef,
        occurrence: Option<&'s OccurrenceDef>,
        element: usize,
        component: Option<usize>,
    ) -> Self {
        let own =
            occurrence.and_then(|o| o.codes.get(&(element, component)).map(|codes| (o, codes)));
        Self {
            codes: own.map_or(def.codes.as_slice(), |(_, codes)| codes.as_slice()),
            codes_of: own.map(|(o, _)| o.name.as_str()),
            ..Self::of(def)
        }
    }

    /// `true` when the rule has a code list and `value` is not in it.
    pub(crate) fn rejects(&self, value: &[u8]) -> bool {
        rejects_code(self.codes, value)
    }
}
