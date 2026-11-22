//! The table chains and declared occurrences a table error's message quotes.

use std::borrow::Cow;
use std::fmt;

use crate::message::{render_key, render_names};

/// Table names outermost first, joined by `/`; `no table` when there are none.
pub(super) fn render_chain(chain: &[String]) -> String {
    if chain.is_empty() {
        return "no table".to_string();
    }
    let names: Vec<Cow<'_, str>> = chain.iter().map(|name| render_key(name)).collect();
    names.join("/")
}

/// The occurrences a loop declares, or that it declares none.
pub(crate) struct Declared<'a>(pub(crate) &'a [String]);

impl fmt::Display for Declared<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return f.write_str("it declares none");
        }
        write!(f, "it declares {}", render_names(self.0))
    }
}
