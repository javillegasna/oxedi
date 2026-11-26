//! Findings about a file's data, each one readable on its own.
//!
//! A diagnostic names the rule that failed, where it failed (segment index,
//! element and component position, and the open loops with the ordinal of
//! each instance) and the offending value as it appears in the file. It holds
//! owned values only, so it can be printed, stored or sent elsewhere without
//! the spec or the document that produced it.
//!
//! The folder holds: this file (the loop and diagnostic types, the SNIP
//! levels), `flat.rs` (the flat record of a diagnostic), `rule.rs` (the rules,
//! their levels and variant names) and `tests.rs`. The message text is in
//! `problem/diagnostic/`.

use std::fmt;

use crate::document::{Document, Span};
use crate::message::render_key;

pub use flat::{FlatDiagnostic, PARSER_ORIGIN};
pub use rule::Rule;

/// The SNIP validation level a rule belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SnipLevel {
    /// Integrity: envelopes, control numbers and counts, segment structure.
    L1,
    /// Requirements: required elements, types and lengths.
    L2,
    /// Balancing: amounts that must add up.
    L3,
}

/// One open loop instance: the loop's name and the 1-based ordinal of the
/// instance among every instance of that loop in the stream.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoopRef {
    /// Loop name as the spec writes it, e.g. `2100`.
    pub name: String,
    /// 1 for the first instance of the loop, 2 for the second, and so on.
    pub ordinal: usize,
}

impl fmt::Display for LoopRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", render_key(&self.name), self.ordinal)
    }
}

/// Open loops written outermost first, joined by `/` (`2000#1/2100#3`); empty
/// for no loop.
pub(crate) struct LoopPath<'a>(pub(crate) &'a [LoopRef]);

impl fmt::Display for LoopPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, open) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str("/")?;
            }
            write!(f, "{open}")?;
        }
        Ok(())
    }
}

/// Writes `path` (loop name and 1-based ordinal, outermost first) on every
/// diagnostic of the slice.
pub(crate) fn locate<'a>(
    diagnostics: &mut [Diagnostic],
    path: impl Iterator<Item = (&'a str, usize)>,
) {
    let path: Vec<LoopRef> = path
        .map(|(name, ordinal)| LoopRef {
            name: name.to_string(),
            ordinal,
        })
        .collect();
    for diagnostic in diagnostics {
        diagnostic.path.clone_from(&path);
    }
}

/// One finding about the data of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// What failed.
    pub rule: Rule,
    /// The SNIP level of `rule`.
    pub level: SnipLevel,
    /// Index of the segment at fault; `None` when the finding is about the
    /// end of the stream.
    pub segment: Option<usize>,
    /// 1-based element position inside the segment, when the finding is.
    pub element: Option<usize>,
    /// 1-based component position inside the element, when the finding is.
    pub component: Option<usize>,
    /// Open loops at the time, outermost first.
    pub path: Vec<LoopRef>,
    /// The offending value as it appears in the file.
    pub datum: Vec<u8>,
}

impl Diagnostic {
    /// A diagnostic whose level is the rule's own.
    pub fn new(
        rule: Rule,
        segment: Option<usize>,
        element: Option<usize>,
        component: Option<usize>,
        path: Vec<LoopRef>,
        datum: Vec<u8>,
    ) -> Diagnostic {
        Diagnostic {
            level: rule.level(),
            rule,
            segment,
            element,
            component,
            path,
            datum,
        }
    }

    /// Where the segment at fault lives in `document`; `None` when the
    /// diagnostic names no segment or the document has no such segment.
    pub fn span(&self, document: &Document<'_>) -> Option<Span> {
        self.segment.and_then(|index| document.span(index))
    }
}

mod flat;
mod rule;

#[cfg(test)]
mod tests;
