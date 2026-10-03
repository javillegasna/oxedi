//! Delimiters of an X12 interchange, read from the ISA segment.
//!
//! The ISA is nominally 106 bytes of fixed width, but real payer files pad
//! ISA06/ISA08 wrongly (105- and 102-byte ISAs exist). So byte offsets are
//! never trusted: element separators are counted instead. The separator right
//! after `ISA` is #1; ISA16 (the component separator) is the single byte after
//! separator #16, and the segment terminator is the byte after that.

use std::fmt;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsaError {
    /// The input does not start with the bytes `ISA`.
    NotIsa,
    /// The input ends before the 16 separators, ISA16 and the terminator.
    Truncated {
        /// Length of the input that was examined.
        len: usize,
    },
}

impl fmt::Display for IsaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IsaError::NotIsa => write!(f, "input does not start with an ISA segment"),
            IsaError::Truncated { len } => write!(f, "ISA segment truncated after {len} bytes"),
        }
    }
}

impl std::error::Error for IsaError {}

/// Number of element separators in an ISA segment (ISA01 through ISA16).
const ISA_SEPARATORS: usize = 16;
/// First interchange version in which ISA11 is a repetition separator.
const FIRST_VERSION_WITH_REPETITION: &[u8] = b"00402";

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
            return Err(IsaError::NotIsa);
        }
        let truncated = IsaError::Truncated { len: input.len() };
        let element = *input.get(3).ok_or(truncated)?;

        let mut separators = input
            .iter()
            .enumerate()
            .skip(3)
            .filter(|&(_, &byte)| byte == element)
            .map(|(at, _)| at);
        let mut at = [0usize; ISA_SEPARATORS];
        for slot in &mut at {
            *slot = separators.next().ok_or(truncated)?;
        }

        let component = *input.get(at[15] + 1).ok_or(truncated)?;
        let segment = *input.get(at[15] + 2).ok_or(truncated)?;

        // ISA11 sits between separators #11 and #12, ISA12 between #12 and #13.
        let isa11 = &input[at[10] + 1..at[11]];
        let isa12 = &input[at[11] + 1..at[12]];
        let repetition = match isa11 {
            [byte] if isa12 >= FIRST_VERSION_WITH_REPETITION => Some(*byte),
            _ => None,
        };

        Ok(Self {
            element,
            component,
            segment,
            repetition,
            release: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verbatim ISA segments from the fixtures. Note the non-standard padding of
    // ISA06/ISA08 in the trizetto one: it is 102 bytes long, not 106.
    const ISA_5010: &[u8] =
        b"ISA*00*          *00*          *ZZ*EMEDNYBAT      *ZZ*ETIN           *100101*1000*^*00501*006000600*0*T*:~";
    const ISA_4010_SHORT: &[u8] =
        b"ISA*00*          *00*          *ZZ*SENDER       *ZZ*RECEIVER     *240416*0930*U*00401*000001234*0*P*>~";

    #[test]
    fn from_isa_reads_all_four_delimiters_of_a_5010_file() {
        let d = Delimiters::from_isa(ISA_5010).unwrap();
        assert_eq!(d.element, b'*');
        assert_eq!(d.component, b':');
        assert_eq!(d.segment, b'~');
        assert_eq!(d.repetition, Some(b'^'));
        assert_eq!(d.release, None);
    }

    #[test]
    fn from_isa_counts_separators_on_short_isa() {
        assert_eq!(
            ISA_4010_SHORT.len(),
            102,
            "fixture-derived ISA must be short"
        );
        let d = Delimiters::from_isa(ISA_4010_SHORT).unwrap();
        assert_eq!(d.component, b'>');
        assert_eq!(d.segment, b'~');
    }

    #[test]
    fn isa11_is_not_a_repetition_separator_before_version_00402() {
        let d = Delimiters::from_isa(ISA_4010_SHORT).unwrap();
        assert_eq!(d.repetition, None);
    }

    #[test]
    fn from_isa_ignores_bytes_after_the_terminator() {
        let mut input = ISA_5010.to_vec();
        input.extend_from_slice(b"GS*HP*X~");
        assert_eq!(Delimiters::from_isa(&input), Delimiters::from_isa(ISA_5010));
    }

    #[test]
    fn from_isa_rejects_input_that_does_not_start_with_isa() {
        assert_eq!(Delimiters::from_isa(b"ST*835*1234~"), Err(IsaError::NotIsa));
        assert_eq!(Delimiters::from_isa(b""), Err(IsaError::NotIsa));
    }

    #[test]
    fn from_isa_reports_truncated_isa() {
        assert_eq!(
            Delimiters::from_isa(b"ISA"),
            Err(IsaError::Truncated { len: 3 })
        );
        assert_eq!(
            Delimiters::from_isa(b"ISA*00*"),
            Err(IsaError::Truncated { len: 7 })
        );
        let cut = &ISA_5010[..ISA_5010.len() - 1]; // terminator missing
        assert_eq!(
            Delimiters::from_isa(cut),
            Err(IsaError::Truncated { len: 105 })
        );
    }

    #[test]
    fn builders_set_optional_delimiters() {
        let d = Delimiters::new(b'|', b':', b'~')
            .with_repetition(b'^')
            .with_release(b'?');
        assert_eq!(d.element, b'|');
        assert_eq!(d.repetition, Some(b'^'));
        assert_eq!(d.release, Some(b'?'));
    }

    #[test]
    fn isa_error_displays_a_message() {
        assert_eq!(
            IsaError::NotIsa.to_string(),
            "input does not start with an ISA segment"
        );
        assert_eq!(
            IsaError::Truncated { len: 7 }.to_string(),
            "ISA segment truncated after 7 bytes"
        );
    }
}
