//! The text of a diagnostic: its line and the SNIP level.
//!
//! The folder holds: this file (the diagnostic's line and the level's name),
//! `rule.rs` (the message text of each rule) and `text.rs` (the pieces those
//! messages are written from, and the diagnostic's location pieces).

use std::fmt;

use crate::diagnostic::{Diagnostic, SnipLevel};
use crate::message::Quoted;
use text::{Location, Within};

mod rule;
mod text;

impl fmt::Display for SnipLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SNIP {}", self.number())
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let location = Location {
            segment: self.segment,
            element: self.element,
            component: self.component,
        };
        write!(
            f,
            "{} · {} · {location} · at {} · datum {}",
            self.level,
            self.rule,
            Within(&self.path),
            Quoted(&self.datum)
        )
    }
}
