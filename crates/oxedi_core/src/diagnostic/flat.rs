//! A diagnostic as flat fields, the shape every binding's table or class
//! of diagnostics shares.

use super::{Diagnostic, Rule, SnipLevel};

/// Who reports the parser's own findings.
pub const PARSER_ORIGIN: &str = "oxedi";

impl SnipLevel {
    /// The level as a number: 1, 2 or 3.
    pub fn number(self) -> u8 {
        match self {
            SnipLevel::L1 => 1,
            SnipLevel::L2 => 2,
            SnipLevel::L3 => 3,
        }
    }
}

/// A diagnostic as ten flat fields, borrowed from it; the columns a table of
/// diagnostics has, in [`FlatDiagnostic::FIELDS`] order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatDiagnostic<'a> {
    /// The SNIP level: 1, 2 or 3.
    pub level: u8,
    /// The name of the rule's variant.
    pub kind: &'static str,
    /// The rule as a sentence with its values.
    pub rule: String,
    /// 0-based index of the segment at fault; `None` at the end of the stream.
    pub segment: Option<usize>,
    /// 1-based element position, when the finding has one.
    pub element: Option<usize>,
    /// 1-based component position, when the finding has one.
    pub component: Option<usize>,
    /// The open loops joined by `/`, e.g. `2000#1/2100#3`; empty at the root.
    pub path: String,
    /// The offending value as it appears in the file.
    pub datum: &'a [u8],
    /// [`PARSER_ORIGIN`] for the parser's own rules, the validator's name for
    /// an external finding.
    pub origin: &'a str,
    /// The external validator's own code, when it gives one.
    pub code: Option<&'a str>,
}

impl FlatDiagnostic<'_> {
    /// The field names, in the order of the struct.
    pub const FIELDS: [&'static str; 10] = [
        "level",
        "kind",
        "rule",
        "segment",
        "element",
        "component",
        "path",
        "datum",
        "origin",
        "code",
    ];
}

impl Diagnostic {
    /// The diagnostic as flat fields.
    pub fn flat(&self) -> FlatDiagnostic<'_> {
        let (origin, code) = match &self.rule {
            Rule::External { origin, code, .. } => (origin.as_str(), code.as_deref()),
            _ => (PARSER_ORIGIN, None),
        };
        FlatDiagnostic {
            level: self.level.number(),
            kind: self.rule.kind(),
            rule: self.rule.to_string(),
            segment: self.segment,
            element: self.element,
            component: self.component,
            path: self
                .path
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("/"),
            datum: &self.datum,
            origin,
            code,
        }
    }
}
