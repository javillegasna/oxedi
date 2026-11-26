//! Masking the diagnostics an external validator raised.

use crate::diagnostic::{Diagnostic, Rule};
use crate::document::Document;
use crate::mask::Mask;
use crate::project::Projector;
use crate::spec::Spec;

/// Masks `found`, diagnostics raised about `document`, as the reader masks
/// its own: the datum at a flagged place, and in the message the datum and
/// each flagged value of the segment. The diagnostic at position `i` is one
/// the caller wrote itself when `own_text[i]` is true (a position past the
/// slice is not); its message holds no value of the file, so it is kept and
/// only its datum is masked.
pub fn mask_diagnostics(
    found: &mut [Diagnostic],
    document: &Document<'_>,
    spec: &Spec,
    mask: &Mask,
    own_text: &[bool],
) {
    let kept: Vec<Option<String>> = found
        .iter_mut()
        .enumerate()
        .map(|(position, diagnostic)| {
            take_message(diagnostic, own_text.get(position) == Some(&true))
        })
        .collect();
    Projector::new(spec, document.delimiters())
        .with_mask(mask)
        .mask_found(found, document);
    for (diagnostic, original) in found.iter_mut().zip(kept) {
        put_message(diagnostic, original);
    }
}

/// The message of an external diagnostic when `own`, left empty meanwhile.
fn take_message(diagnostic: &mut Diagnostic, own: bool) -> Option<String> {
    match &mut diagnostic.rule {
        Rule::External { message, .. } if own => Some(std::mem::take(message)),
        _ => None,
    }
}

/// Puts back a message `take_message` took.
fn put_message(diagnostic: &mut Diagnostic, original: Option<String>) {
    if let (Rule::External { message, .. }, Some(original)) = (&mut diagnostic.rule, original) {
        *message = original;
    }
}
