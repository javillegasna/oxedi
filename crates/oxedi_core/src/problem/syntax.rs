//! The text of the interchange header's problems, of a document that cannot be
//! parsed and of a segment that cannot be written.

use std::fmt;

use crate::delimiters::{HeaderPositions, ISA_ID, IsaError};
use crate::document::{DocumentError, SizeError};
use crate::message::Counted;
use crate::segment::SegmentWriteError;

impl fmt::Display for IsaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IsaError::NotIsa {
                found,
                byte_order_mark,
                whitespace,
            } => write!(
                f,
                "input does not start with an {ISA_ID} segment ({})",
                NotIsaDetail {
                    found,
                    skipped: Skipped::of(*byte_order_mark, *whitespace),
                }
            ),
            IsaError::Truncated {
                len,
                separators_found,
                byte_order_mark,
                whitespace,
            } => write!(
                f,
                "{ISA_ID} segment truncated after {len} bytes{}: {}",
                CountedAfter(Skipped::of(*byte_order_mark, *whitespace)),
                Missing(*separators_found)
            ),
        }
    }
}

/// The leading bytes skipped before looking for the ISA, as written in
/// [`IsaError::NotIsa`] and [`IsaError::Truncated`] messages.
struct Skipped {
    byte_order_mark: bool,
    whitespace: usize,
}

impl Skipped {
    const fn of(byte_order_mark: bool, whitespace: usize) -> Skipped {
        Skipped {
            byte_order_mark,
            whitespace,
        }
    }

    const fn is_empty(&self) -> bool {
        !self.byte_order_mark && self.whitespace == 0
    }
}

impl fmt::Display for Skipped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let whitespace = Counted(self.whitespace, "byte", "bytes");
        match (self.byte_order_mark, self.whitespace) {
            (true, 0) => write!(f, "a UTF-8 byte order mark"),
            (true, _) => write!(f, "a UTF-8 byte order mark and {whitespace} of whitespace"),
            (false, _) => write!(f, "{whitespace} of whitespace"),
        }
    }
}

/// The bytes found instead of the ISA, as hex digits: `found bytes [ef bb bf]`.
struct Found<'a>(&'a [u8]);

impl fmt::Display for Found<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "found bytes [")?;
        for (i, byte) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{byte:02x}")?;
        }
        write!(f, "]")
    }
}

/// What a missing ISA is told with: what was found instead, and what was
/// skipped before it.
struct NotIsaDetail<'a> {
    found: &'a [u8],
    skipped: Skipped,
}

impl fmt::Display for NotIsaDetail<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.found.is_empty(), self.skipped.is_empty()) {
            (true, true) => write!(f, "input is empty"),
            (true, false) => write!(f, "input holds only {}", self.skipped),
            (false, true) => write!(f, "{}", Found(self.found)),
            (false, false) => write!(f, "{} after skipping {}", Found(self.found), self.skipped),
        }
    }
}

/// The bytes skipped before the count of a truncated ISA, or nothing when none
/// was skipped.
struct CountedAfter(Skipped);

impl fmt::Display for CountedAfter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return Ok(());
        }
        write!(f, " (counted after skipping {})", self.0)
    }
}

/// What a truncated ISA lacks, from the element separators found in it.
struct Missing(usize);

impl fmt::Display for Missing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let component = HeaderPositions::ISA.component;
        if self.0 >= component {
            write!(f, "{ISA_ID}{component} or the terminator is missing")
        } else {
            write!(f, "found {} of {component} element separators", self.0)
        }
    }
}

impl fmt::Display for SizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "input of {} bytes exceeds the document size limit of {} bytes \
             (segment offsets are stored as 32-bit integers)",
            self.len,
            Self::LIMIT
        )
    }
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentError::Isa(e) => write!(f, "{e}"),
            DocumentError::Size(e) => write!(f, "{e}"),
        }
    }
}

impl fmt::Display for SegmentWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SegmentWriteError::Io(e) => write!(f, "write failed: {e}"),
            SegmentWriteError::DelimiterInValue {
                byte,
                element,
                component,
            } => {
                match (element, component) {
                    (0, _) => write!(f, "segment id")?,
                    (element, None) => write!(f, "element {element}")?,
                    (element, Some(component)) => {
                        write!(f, "element {element} component {component}")?;
                    }
                }
                write!(
                    f,
                    " contains delimiter byte 0x{byte:02X} and no release byte is configured"
                )
            }
        }
    }
}
