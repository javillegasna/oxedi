use crate::frame::SegmentEnd;

/// The bytes a segment is written with. The terminator and the release byte
/// are the [`SegmentEnd`] the reading side already names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Syntax {
    pub(crate) element: u8,
    pub(crate) component: u8,
    pub(crate) end: SegmentEnd,
}

impl Syntax {
    /// `true` when `byte` cannot be written as data: the bytes the tokenizer
    /// splits on, plus the release byte itself. The repetition separator is
    /// not split on, so it is data.
    pub(crate) fn is_structural(&self, byte: u8) -> bool {
        byte == self.element
            || byte == self.component
            || byte == self.end.terminator
            || self.end.release == Some(byte)
    }

    /// The first structural byte of `bytes`, if any.
    pub(crate) fn first_structural(&self, bytes: &[u8]) -> Option<u8> {
        bytes.iter().copied().find(|&byte| self.is_structural(byte))
    }
}
