//! Delimiters of an X12 interchange, read from the ISA segment.
//!
//! The ISA is nominally 106 bytes of fixed width, but real payer files pad
//! ISA06/ISA08 wrongly (105- and 102-byte ISAs exist). So byte offsets are
//! never trusted: element separators are counted instead. The separator right
//! after `ISA` is #1; ISA16 (the component separator) is the single byte after
//! separator #16, and the segment terminator is the byte after that.
//!
//! `header` holds the element positions of the ISA that carry the delimiters
//! and the version, and the version from which ISA11 is a repetition separator.
//! `syntax` holds the `Syntax` value, the bytes a segment is written with.
//! `test_support` (test builds only) holds the helpers the unit tests of the
//! modules that read segments share: the plain `*`, `:`, `~` dialect and the
//! concatenation of every segment's `raw` bytes.

use crate::frame::SegmentEnd;
use crate::trivia::LeadingTrivia;

mod header;
mod syntax;

pub(crate) use header::{HeaderPositions, ISA_ID, version_carries_repetition};
pub(crate) use syntax::Syntax;

/// The five delimiters of an interchange. Only `release` is never read from the
/// file: X12 defines no release character, so it is opt-in by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delimiters {
    /// Separates elements inside a segment (usually `*`).
    pub element: u8,
    /// Separates components inside a composite element (`:` or `>`).
    pub component: u8,
    /// Ends a segment (usually `~`).
    pub segment: u8,
    /// Separates repetitions of an element (`^` from version 00402 on).
    pub repetition: Option<u8>,
    /// Makes the following byte literal. Opt-in; never inferred from the file.
    pub release: Option<u8>,
}

/// Why an ISA segment could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IsaError {
    /// The input does not start with the bytes `ISA`.
    NotIsa {
        /// The first bytes after the skipped leading bytes, at most
        /// [`IsaError::FOUND_LEN`]. Empty when nothing follows them.
        found: Vec<u8>,
        /// `true` when a UTF-8 byte order mark at the start was skipped.
        byte_order_mark: bool,
        /// Whitespace bytes skipped after the mark (or from the start).
        whitespace: usize,
    },
    /// The input ends before the 16 separators, ISA16 and the terminator.
    Truncated {
        /// Length of the input that was examined.
        len: usize,
        /// Element separators seen before the input ended, `0..=16`.
        separators_found: usize,
        /// `true` when a UTF-8 byte order mark at the start was skipped.
        byte_order_mark: bool,
        /// Whitespace bytes skipped after the mark (or from the start).
        whitespace: usize,
    },
}

impl IsaError {
    /// How many leading bytes [`IsaError::NotIsa`] keeps.
    pub const FOUND_LEN: usize = 8;

    /// The nominal length of an ISA segment, the most bytes
    /// [`datum`](Self::datum) keeps of a truncated one.
    pub const ISA_LEN: usize = 106;

    /// How many bytes precede the ISA in the input the error was read from:
    /// the byte order mark and the whitespace skipped.
    pub fn skipped(&self) -> usize {
        let (byte_order_mark, whitespace) = match self {
            IsaError::NotIsa {
                byte_order_mark,
                whitespace,
                ..
            }
            | IsaError::Truncated {
                byte_order_mark,
                whitespace,
                ..
            } => (*byte_order_mark, *whitespace),
        };
        LeadingTrivia {
            byte_order_mark,
            whitespace,
        }
        .len()
    }

    /// The bytes the error is about in `input`, the bytes it was read from:
    /// what was found instead of `ISA`, or the first [`ISA_LEN`](Self::ISA_LEN)
    /// bytes after the skipped ones of a truncated ISA.
    pub fn datum<'a>(&'a self, input: &'a [u8]) -> &'a [u8] {
        match self {
            IsaError::NotIsa { found, .. } => found,
            IsaError::Truncated { .. } => {
                let rest = input.get(self.skipped()..).unwrap_or_default();
                rest.get(..Self::ISA_LEN).unwrap_or(rest)
            }
        }
    }
}

impl std::error::Error for IsaError {}

/// Number of element separators in an ISA segment: one before each element
/// (ISA01 through ISA16), ISA16 being the byte after the last.
const ISA_SEPARATORS: usize = HeaderPositions::ISA.component;

/// The positions of the 16 element separators of an ISA segment, found once,
/// so each field is read from them instead of being searched again, with the
/// component separator and the segment terminator that follow the last one.
struct IsaSeparators<'a> {
    input: &'a [u8],
    element: u8,
    component: u8,
    segment: u8,
    at: [usize; ISA_SEPARATORS],
}

impl<'a> IsaSeparators<'a> {
    /// Locates the separators of the ISA at the start of `input`, which
    /// begins with `ISA`; the error is how many separators were found before
    /// the input ended, `ISA_SEPARATORS` when ISA16 or the terminator is
    /// missing.
    fn locate(input: &'a [u8]) -> Result<Self, usize> {
        let element = *input.get(3).ok_or(0usize)?;
        let mut found = input
            .iter()
            .enumerate()
            .skip(3)
            .filter(|&(_, &byte)| byte == element)
            .map(|(at, _)| at);
        let mut at = [0usize; ISA_SEPARATORS];
        for (count, slot) in at.iter_mut().enumerate() {
            *slot = found.next().ok_or(count)?;
        }
        let last = at[ISA_SEPARATORS - 1];
        let component = *input.get(last + 1).ok_or(ISA_SEPARATORS)?;
        let segment = *input.get(last + 2).ok_or(ISA_SEPARATORS)?;
        Ok(Self {
            input,
            element,
            component,
            segment,
            at,
        })
    }

    /// The bytes of ISA`number` (1-based, `1..=15`): between separators
    /// #`number` and #`number + 1`. Empty for any other number.
    fn field(&self, number: usize) -> &'a [u8] {
        let start = number.checked_sub(1).and_then(|at| self.at.get(at));
        let end = self.at.get(number);
        match (start, end) {
            (Some(&start), Some(&end)) => self.input.get(start + 1..end).unwrap_or_default(),
            _ => &[],
        }
    }
}

impl Delimiters {
    /// Delimiters with no repetition and no release byte.
    pub const fn new(element: u8, component: u8, segment: u8) -> Self {
        Self {
            element,
            component,
            segment,
            repetition: None,
            release: None,
        }
    }

    /// Sets the repetition separator.
    #[must_use]
    pub const fn with_repetition(mut self, repetition: u8) -> Self {
        self.repetition = Some(repetition);
        self
    }

    /// Sets the release byte.
    #[must_use]
    pub const fn with_release(mut self, release: u8) -> Self {
        self.release = Some(release);
        self
    }

    /// The bytes that end a segment under these delimiters: the segment
    /// terminator and the release byte, when there is one.
    #[must_use]
    pub const fn segment_end(&self) -> SegmentEnd {
        SegmentEnd::new(self.segment, self.release)
    }

    /// The bytes a segment is written with under these delimiters.
    pub(crate) const fn syntax(&self) -> Syntax {
        Syntax {
            element: self.element,
            component: self.component,
            end: self.segment_end(),
        }
    }

    /// Reads the delimiters from an ISA segment at the start of `input`.
    ///
    /// Bytes after the ISA terminator are ignored. `release` is always `None`.
    pub fn from_isa(input: &[u8]) -> Result<Self, IsaError> {
        if !input.starts_with(ISA_ID.as_bytes()) {
            let found = input.iter().take(IsaError::FOUND_LEN).copied().collect();
            return Err(IsaError::NotIsa {
                found,
                byte_order_mark: false,
                whitespace: 0,
            });
        }
        let isa = IsaSeparators::locate(input).map_err(|separators_found| IsaError::Truncated {
            len: input.len(),
            separators_found,
            byte_order_mark: false,
            whitespace: 0,
        })?;
        let repetition = match isa.field(HeaderPositions::ISA.repetition) {
            [byte] if version_carries_repetition(isa.field(HeaderPositions::ISA.version)) => {
                Some(*byte)
            }
            _ => None,
        };

        Ok(Self {
            element: isa.element,
            component: isa.component,
            segment: isa.segment,
            repetition,
            release: None,
        })
    }

    /// Reads the delimiters from the ISA segment that follows the input's
    /// leading trivia: a UTF-8 byte order mark at the very start, then
    /// whitespace. A [`IsaError::NotIsa`] or [`IsaError::Truncated`] names what was skipped.
    pub fn from_isa_after_leading_trivia(input: &[u8]) -> Result<Self, IsaError> {
        let trivia = LeadingTrivia::of(input);
        let LeadingTrivia {
            byte_order_mark,
            whitespace,
        } = trivia;
        let skipped = trivia.len();
        let rest = input.get(skipped..).unwrap_or_default();
        Self::from_isa(rest).map_err(|error| match error {
            IsaError::NotIsa { found, .. } => IsaError::NotIsa {
                found,
                byte_order_mark,
                whitespace,
            },
            IsaError::Truncated {
                len,
                separators_found,
                ..
            } => IsaError::Truncated {
                len,
                separators_found,
                byte_order_mark,
                whitespace,
            },
        })
    }

    /// `true` when `byte` must be escaped on output: the delimiters the tokenizer
    /// splits on, plus the release byte itself. The repetition separator is not
    /// split on, so it is written as data (ISA11 itself contains it).
    pub fn is_special(&self, byte: u8) -> bool {
        self.syntax().is_structural(byte)
    }
}

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
