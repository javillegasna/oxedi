//! The closed words and the codes key a spec writes, one table each: the list
//! of words of a vocabulary, the word of each value and the value of each
//! word, so the loader reads every word through one place.

use std::fmt;

use super::balance::Sign;
use super::loops::ControlCount;
use super::occurrences::Usage;
use super::segments::parse_position;
use crate::mask::Phi;

/// A closed set of words a spec writes for a typed value.
pub(crate) trait Vocabulary: Copy + Eq + 'static {
    /// Every value, in the order a spec lists them.
    const ALL: &'static [Self];

    /// The word a spec writes for the value.
    fn word(self) -> &'static str;

    /// The value a spec's word names, or `None` when the word is not in
    /// the vocabulary; matching is exact.
    fn from_word(word: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|value| value.word() == word)
    }
}

impl Vocabulary for Usage {
    const ALL: &'static [Usage] = &[Usage::Required, Usage::Situational];

    fn word(self) -> &'static str {
        match self {
            Usage::Required => "required",
            Usage::Situational => "situational",
        }
    }
}

impl Vocabulary for ControlCount {
    const ALL: &'static [ControlCount] = &[ControlCount::Segments, ControlCount::Children];

    fn word(self) -> &'static str {
        match self {
            ControlCount::Segments => "segments",
            ControlCount::Children => "children",
        }
    }
}

impl Vocabulary for Sign {
    const ALL: &'static [Sign] = &[Sign::Plus, Sign::Minus];

    fn word(self) -> &'static str {
        match self {
            Sign::Plus => "+",
            Sign::Minus => "-",
        }
    }
}

impl Vocabulary for Phi {
    const ALL: &'static [Phi] = &Phi::ALL;

    fn word(self) -> &'static str {
        Phi::word(self)
    }
}

/// The position a `codes` key names: an element and maybe one of its
/// components, written `<element>` or `<element>-<component>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElementPosition {
    /// 1-based element position.
    pub element: usize,
    /// 1-based component position inside the element, when the key names one.
    pub component: Option<usize>,
}

impl ElementPosition {
    /// Reads a key in canonical form (no sign, no leading zero, no position
    /// 0); `None` for anything else.
    pub fn parse(key: &str) -> Option<ElementPosition> {
        let (element, component) = match key.split_once('-') {
            None => (key, None),
            Some((element, component)) => (element, Some(parse_position(component)?)),
        };
        Some(ElementPosition {
            element: parse_position(element)?,
            component,
        })
    }
}

impl fmt::Display for ElementPosition {
    /// The key as a spec writes it: `3` or `3-2`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.component {
            None => write!(f, "{}", self.element),
            Some(component) => write!(f, "{}-{component}", self.element),
        }
    }
}
