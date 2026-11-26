//! The pieces the texts of the problems are written from: a count and its
//! noun (`Counted`), a trigger's conditions (`Conditions`), a list of names
//! (`render_names`) and the longest code list a text spells out
//! (`MAX_LISTED_CODES`).

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
