//! Delimiters of an X12 interchange, read from the ISA segment.
//!
//! The ISA is nominally 106 bytes of fixed width, but real payer files pad
//! ISA06/ISA08 wrongly (105- and 102-byte ISAs exist). So byte offsets are
//! never trusted: element separators are counted instead. The separator right
//! after `ISA` is #1; ISA16 (the component separator) is the single byte after
//! separator #16, and the segment terminator is the byte after that.
//!
//! `test_support` (test builds only) holds the helpers the unit tests of the
//! modules that read segments share: the plain `*`, `:`, `~` dialect and the
//! concatenation of every segment's `raw` bytes.

use std::fmt;

use crate::trivia::LeadingTrivia;

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

impl fmt::Display for IsaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IsaError::NotIsa {
                found,
                byte_order_mark,
                whitespace,
            } => {
                let skipped = Skipped {
                    byte_order_mark: *byte_order_mark,
                    whitespace: *whitespace,
                };
                write!(f, "input does not start with an ISA segment (")?;
                if found.is_empty() {
                    if skipped.is_empty() {
                        return write!(f, "input is empty)");
                    }
                    return write!(f, "input holds only {skipped})");
                }
                write!(f, "found bytes [")?;
                for (i, byte) in found.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{byte:02x}")?;
                }
                write!(f, "]")?;
                if !skipped.is_empty() {
                    write!(f, " after skipping {skipped}")?;
                }
                write!(f, ")")
            }
            IsaError::Truncated {
                len,
                separators_found,
                byte_order_mark,
                whitespace,
            } => {
                let skipped = Skipped {
                    byte_order_mark: *byte_order_mark,
                    whitespace: *whitespace,
                };
                write!(f, "ISA segment truncated after {len} bytes")?;
                if !skipped.is_empty() {
                    write!(f, " (counted after skipping {skipped})")?;
                }
                if *separators_found >= ISA_SEPARATORS {
                    write!(f, ": ISA16 or the terminator is missing")
                } else {
                    write!(
                        f,
                        ": found {separators_found} of {ISA_SEPARATORS} element separators"
                    )
                }
            }
        }
    }
}

impl std::error::Error for IsaError {}

/// The leading bytes skipped before looking for the ISA, as written in
/// [`IsaError::NotIsa`] and [`IsaError::Truncated`] messages.
struct Skipped {
    byte_order_mark: bool,
    whitespace: usize,
}

impl Skipped {
    const fn is_empty(&self) -> bool {
        !self.byte_order_mark && self.whitespace == 0
    }
}

impl fmt::Display for Skipped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let unit = if self.whitespace == 1 {
            "byte"
        } else {
            "bytes"
        };
        match (self.byte_order_mark, self.whitespace) {
            (true, 0) => write!(f, "a UTF-8 byte order mark"),
            (true, n) => write!(f, "a UTF-8 byte order mark and {n} {unit} of whitespace"),
            (false, n) => write!(f, "{n} {unit} of whitespace"),
        }
    }
}

/// Number of element separators in an ISA segment (ISA01 through ISA16).
const ISA_SEPARATORS: usize = 16;
/// First interchange version in which ISA11 is a repetition separator.
const FIRST_VERSION_WITH_REPETITION: &[u8] = b"00402";

/// `true` when an interchange of `version` (ISA12) uses ISA11 as the
/// repetition separator. The comparison is on bytes, so a version that is not
/// a five-digit number is ordered as its bytes are.
pub(crate) fn version_carries_repetition(version: &[u8]) -> bool {
    version >= FIRST_VERSION_WITH_REPETITION
}

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

    /// Reads the delimiters from an ISA segment at the start of `input`.
    ///
    /// Bytes after the ISA terminator are ignored. `release` is always `None`.
    pub fn from_isa(input: &[u8]) -> Result<Self, IsaError> {
        if !input.starts_with(b"ISA") {
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
        let repetition = match isa.field(11) {
            [byte] if version_carries_repetition(isa.field(12)) => Some(*byte),
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
        byte == self.element
            || byte == self.component
            || byte == self.segment
            || self.release == Some(byte)
    }
}

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
