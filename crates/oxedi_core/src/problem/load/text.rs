//! The parts of the load error's text that are built from lists.

use std::fmt;

use crate::problem::text::Conditions;

/// The parent of sibling loops as a message names it: quoted, or `the root`.
pub(super) struct Under<'a>(pub(super) Option<&'a str>);

impl fmt::Display for Under<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(parent) => write!(f, "{parent:?}"),
            None => write!(f, "the root"),
        }
    }
}

/// The loops of a parent cycle in walk order, back to the first, e.g.
/// `loops form a parent cycle: a -> b -> a`.
pub(super) fn write_cycle(f: &mut fmt::Formatter<'_>, members: &[String]) -> fmt::Result {
    let mut walk: Vec<&str> = members.iter().map(String::as_str).collect();
    walk.extend(members.first().map(String::as_str));
    write!(f, "loops form a parent cycle: {}", walk.join(" -> "))
}

/// Two sibling loops that share one trigger, with its conditions when it has any.
pub(super) fn write_ambiguous(
    f: &mut fmt::Formatter<'_>,
    [first, second]: [&String; 2],
    parent: Option<&str>,
    segment: &str,
    conditions: &[(usize, String)],
) -> fmt::Result {
    write!(
        f,
        "loops {first:?} and {second:?} under {} share the identical trigger {segment:?}",
        Under(parent)
    )?;
    write!(f, "{}", Conditions(conditions))
}
