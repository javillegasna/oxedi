//! The pieces messages are written from, shared by the diagnostics, the spec
//! errors, the write findings and the project errors: file bytes quoted on
//! one line (`Quoted`), a key in a rendered path (`render_key`), an element
//! reference (`ElementRef`, the one public item), a trigger's conditions
//! (`Conditions`), a list of names (`render_names`), a count and its noun
//! (`Counted`) and the longest code list a message spells out
//! (`MAX_LISTED_CODES`).
//!
//! The module depends on nothing else in the crate, so every part of the
//! crate that writes a message can use it without a cycle.

use std::borrow::Cow;
use std::fmt;

/// The longest code list a message spells out; a longer list gives its count.
pub(crate) const MAX_LISTED_CODES: usize = 5;

/// A count and its noun, in the singular for one and the plural otherwise.
pub(crate) struct Counted(
    pub(crate) usize,
    pub(crate) &'static str,
    pub(crate) &'static str,
);

impl fmt::Display for Counted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let noun = if self.0 == 1 { self.1 } else { self.2 };
        write!(f, "{} {noun}", self.0)
    }
}

/// Bytes from the file, quoted on one line: valid text is escaped as a Rust
/// string literal is, and each invalid byte is written as `\xNN`.
pub(crate) struct Quoted<'a>(pub(crate) &'a [u8]);

impl fmt::Display for Quoted<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("\"")?;
        for chunk in self.0.utf8_chunks() {
            let escaped = format!("{:?}", chunk.valid());
            let inner = escaped
                .strip_prefix('"')
                .and_then(|text| text.strip_suffix('"'))
                .unwrap_or(&escaped);
            f.write_str(inner)?;
            for byte in chunk.invalid() {
                write!(f, "\\x{byte:02X}")?;
            }
        }
        f.write_str("\"")
    }
}

/// A key as it appears in a rendered path: unchanged, or JSON-quoted when it
/// is empty or holds a path separator (`.`, `/`, `#`) or whitespace, which
/// would make the path ambiguous.
pub(crate) fn render_key(key: &str) -> Cow<'_, str> {
    let ambiguous = key.is_empty()
        || key
            .chars()
            .any(|c| matches!(c, '.' | '/' | '#') || c.is_whitespace());
    if !ambiguous {
        return Cow::Borrowed(key);
    }
    Cow::Owned(serde_json::to_string(key).unwrap_or_else(|_| format!("{key:?}")))
}

/// An element reference in X12 style: `CLP01`, or `SVC01-2` for a component.
/// It owns its segment id (a `Vec<u8>`) or borrows it (any `AsRef<[u8]>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementRef<S = Vec<u8>> {
    segment_id: S,
    element: usize,
    component: Option<usize>,
}

impl<S> ElementRef<S> {
    /// A simple element, or a component when `component` is `Some`.
    pub(crate) fn new(segment_id: S, element: usize, component: Option<usize>) -> Self {
        ElementRef {
            segment_id,
            element,
            component,
        }
    }

    /// A simple element.
    pub(crate) fn simple(segment_id: S, element: usize) -> Self {
        ElementRef::new(segment_id, element, None)
    }

    /// The segment id.
    pub fn segment_id(&self) -> &S {
        &self.segment_id
    }

    /// The 1-based element position.
    pub fn element(&self) -> usize {
        self.element
    }

    /// The 1-based component position, when the reference is a component.
    pub fn component(&self) -> Option<usize> {
        self.component
    }
}

impl<S: AsRef<[u8]>> fmt::Display for ElementRef<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{:02}",
            String::from_utf8_lossy(self.segment_id.as_ref()),
            self.element
        )?;
        match self.component {
            Some(component) => write!(f, "-{component}"),
            None => Ok(()),
        }
    }
}

/// The conditions of a trigger as ` where {1: "PR", 2: "X"}` (with the
/// leading space), or nothing when there are none. Values are read lossily.
pub(crate) struct Conditions<'a, V>(pub(crate) &'a [(usize, V)]);

impl<V: AsRef<[u8]>> fmt::Display for Conditions<'_, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return Ok(());
        }
        f.write_str(" where {")?;
        for (index, (position, value)) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write!(
                f,
                "{position}: {:?}",
                String::from_utf8_lossy(value.as_ref())
            )?;
        }
        f.write_str("}")
    }
}

/// Names quoted and joined with commas.
pub(crate) fn render_names<S: AsRef<str>>(names: &[S]) -> String {
    names
        .iter()
        .map(|name| format!("{:?}", name.as_ref()))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests;
