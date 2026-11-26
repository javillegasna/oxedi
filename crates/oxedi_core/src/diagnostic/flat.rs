//! A diagnostic's own values as plain ones: the level as a number, who
//! reported it, its code and its path text.

use super::{Diagnostic, LoopPath, Rule, SnipLevel};

/// Who reports the parser's own diagnostics.
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

impl Diagnostic {
    /// Who reported the diagnostic: [`PARSER_ORIGIN`] for the parser's own
    /// rules, the validator's name for an external one.
    pub fn origin(&self) -> &str {
        match &self.rule {
            Rule::External { origin, .. } => origin,
            _ => PARSER_ORIGIN,
        }
    }

    /// The external validator's own code, when the diagnostic has one.
    pub fn code(&self) -> Option<&str> {
        match &self.rule {
            Rule::External { code, .. } => code.as_deref(),
            _ => None,
        }
    }

    /// The open loops joined by `/`, outermost first, e.g. `2000#1/2100#3`;
    /// empty at the root.
    pub fn path_text(&self) -> String {
        LoopPath(&self.path).to_string()
    }
}
