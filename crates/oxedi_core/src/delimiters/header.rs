//! Where the interchange header holds what the delimiters depend on.

/// 1-based element positions of the interchange header (the ISA) that carry
/// the delimiters and the version, shared by the reader and the writer.
///
/// The reader takes the component separator as the byte after the last
/// element separator, so `component` is also the number of element separators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HeaderPositions {
    /// The element that holds the repetition separator (ISA11).
    pub(crate) repetition: usize,
    /// The element that holds the interchange version (ISA12).
    pub(crate) version: usize,
    /// The element that holds the component separator (ISA16).
    pub(crate) component: usize,
}

impl HeaderPositions {
    /// The positions in an ISA segment.
    pub(crate) const ISA: HeaderPositions = HeaderPositions {
        repetition: 11,
        version: 12,
        component: 16,
    };
}

/// First interchange version in which ISA11 is a repetition separator.
const FIRST_VERSION_WITH_REPETITION: &[u8] = b"00402";

/// `true` when an interchange of `version` (ISA12) uses ISA11 as the
/// repetition separator. The comparison is on bytes, so a version that is not
/// a five-digit number is ordered as its bytes are.
pub(crate) fn version_carries_repetition(version: &[u8]) -> bool {
    version >= FIRST_VERSION_WITH_REPETITION
}
