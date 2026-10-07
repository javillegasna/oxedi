//! Helpers shared by the unit tests of the modules that read segments.

use crate::{Delimiters, Segment};

/// The delimiters of the plain test dialect: `*`, `:` and `~`, no release.
pub(crate) fn plain() -> Delimiters {
    Delimiters::new(b'*', b':', b'~')
}

/// Every segment's `raw` bytes, back to back.
pub(crate) fn concat_raw(segments: &[Segment<'_>]) -> Vec<u8> {
    segments
        .iter()
        .flat_map(|s| s.raw.iter().copied())
        .collect()
}
