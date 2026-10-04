//! Delimiters of an X12 interchange, read from the ISA segment.
//!
//! The ISA is nominally 106 bytes of fixed width, but real payer files pad
//! ISA06/ISA08 wrongly (105- and 102-byte ISAs exist). So byte offsets are
//! never trusted: element separators are counted instead. The separator right
//! after `ISA` is #1; ISA16 (the component separator) is the single byte after
//! separator #16, and the segment terminator is the byte after that.

use std::fmt;

use crate::frame::{BYTE_ORDER_MARK, leading_trivia};

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
    },
}

impl IsaError {
    /// How many leading bytes [`IsaError::NotIsa`] keeps.
    pub const FOUND_LEN: usize = 8;
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
            } if *separators_found >= ISA_SEPARATORS => write!(
                f,
                "ISA segment truncated after {len} bytes: ISA16 or the terminator is missing"
            ),
            IsaError::Truncated {
                len,
                separators_found,
            } => write!(
                f,
                "ISA segment truncated after {len} bytes: found {separators_found} of {ISA_SEPARATORS} element separators"
            ),
        }
    }
}

impl std::error::Error for IsaError {}

/// The leading bytes skipped before looking for the ISA, as written in
/// [`IsaError::NotIsa`] messages.
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
        let truncated = |separators_found| IsaError::Truncated {
            len: input.len(),
            separators_found,
        };
        let element = *input.get(3).ok_or_else(|| truncated(0))?;

        let mut separators = input
            .iter()
            .enumerate()
            .skip(3)
            .filter(|&(_, &byte)| byte == element)
            .map(|(at, _)| at);
        let mut at = [0usize; ISA_SEPARATORS];
        for (found, slot) in at.iter_mut().enumerate() {
            *slot = separators.next().ok_or_else(|| truncated(found))?;
        }

        let component = *input
            .get(at[15] + 1)
            .ok_or_else(|| truncated(ISA_SEPARATORS))?;
        let segment = *input
            .get(at[15] + 2)
            .ok_or_else(|| truncated(ISA_SEPARATORS))?;

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

    /// Reads the delimiters from the ISA segment that follows the input's
    /// leading trivia: a UTF-8 byte order mark at the very start, then
    /// whitespace. A [`IsaError::NotIsa`] names what was skipped.
    pub fn from_isa_after_leading_trivia(input: &[u8]) -> Result<Self, IsaError> {
        let skipped = leading_trivia(input);
        let byte_order_mark = input.starts_with(BYTE_ORDER_MARK);
        let whitespace = if byte_order_mark {
            skipped.saturating_sub(BYTE_ORDER_MARK.len())
        } else {
            skipped
        };
        let rest = input.get(skipped..).unwrap_or_default();
        Self::from_isa(rest).map_err(|error| match error {
            IsaError::NotIsa { found, .. } => IsaError::NotIsa {
                found,
                byte_order_mark,
                whitespace,
            },
            truncated @ IsaError::Truncated { .. } => truncated,
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
        assert_eq!(
            Delimiters::from_isa(b"ST*835*1234~"),
            Err(IsaError::NotIsa {
                found: b"ST*835*1".to_vec(),
                byte_order_mark: false,
                whitespace: 0,
            })
        );
        assert_eq!(
            Delimiters::from_isa(b"GS"),
            Err(IsaError::NotIsa {
                found: b"GS".to_vec(),
                byte_order_mark: false,
                whitespace: 0,
            })
        );
        assert_eq!(
            Delimiters::from_isa(b""),
            Err(IsaError::NotIsa {
                found: Vec::new(),
                byte_order_mark: false,
                whitespace: 0,
            })
        );
    }

    #[test]
    fn from_isa_reports_truncated_isa() {
        assert_eq!(
            Delimiters::from_isa(b"ISA"),
            Err(IsaError::Truncated {
                len: 3,
                separators_found: 0
            })
        );
        assert_eq!(
            Delimiters::from_isa(b"ISA*00*"),
            Err(IsaError::Truncated {
                len: 7,
                separators_found: 2
            })
        );
        let cut = &ISA_5010[..ISA_5010.len() - 1]; // terminator missing
        assert_eq!(
            Delimiters::from_isa(cut),
            Err(IsaError::Truncated {
                len: 105,
                separators_found: 16
            })
        );
        let cut = &ISA_5010[..ISA_5010.len() - 2]; // ISA16 and terminator missing
        assert_eq!(
            Delimiters::from_isa(cut),
            Err(IsaError::Truncated {
                len: 104,
                separators_found: 16
            })
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
    fn not_isa_displays_the_leading_bytes_in_hex() {
        assert_eq!(
            IsaError::NotIsa {
                found: b"\xEF\xBB\xBFISA*0".to_vec(),
                byte_order_mark: false,
                whitespace: 0,
            }
            .to_string(),
            "input does not start with an ISA segment (found bytes [ef bb bf 49 53 41 2a 30])"
        );
        assert_eq!(
            IsaError::NotIsa {
                found: Vec::new(),
                byte_order_mark: false,
                whitespace: 0,
            }
            .to_string(),
            "input does not start with an ISA segment (input is empty)"
        );
    }

    #[test]
    fn not_isa_displays_what_was_skipped_before_the_found_bytes() {
        let not_isa = |found: &[u8], byte_order_mark, whitespace| IsaError::NotIsa {
            found: found.to_vec(),
            byte_order_mark,
            whitespace,
        };
        assert_eq!(
            not_isa(b"", true, 0).to_string(),
            "input does not start with an ISA segment (input holds only a UTF-8 byte order mark)"
        );
        assert_eq!(
            not_isa(b"", true, 1).to_string(),
            "input does not start with an ISA segment (input holds only a UTF-8 byte order mark and 1 byte of whitespace)"
        );
        assert_eq!(
            not_isa(b"", false, 4).to_string(),
            "input does not start with an ISA segment (input holds only 4 bytes of whitespace)"
        );
        assert_eq!(
            not_isa(b"GS*HP~", true, 0).to_string(),
            "input does not start with an ISA segment (found bytes [47 53 2a 48 50 7e] after skipping a UTF-8 byte order mark)"
        );
        assert_eq!(
            not_isa(b"GS", true, 2).to_string(),
            "input does not start with an ISA segment (found bytes [47 53] after skipping a UTF-8 byte order mark and 2 bytes of whitespace)"
        );
        assert_eq!(
            not_isa(b"GS", false, 1).to_string(),
            "input does not start with an ISA segment (found bytes [47 53] after skipping 1 byte of whitespace)"
        );
    }

    #[test]
    fn from_isa_after_leading_trivia_names_the_skipped_bytes() {
        let read = |input: &[u8]| {
            Delimiters::from_isa_after_leading_trivia(input)
                .err()
                .map(|error| error.to_string())
        };
        assert_eq!(
            read(b"\xEF\xBB\xBF").as_deref(),
            Some(
                "input does not start with an ISA segment (input holds only a UTF-8 byte order mark)"
            )
        );
        assert_eq!(
            read(b" \r\n\t").as_deref(),
            Some(
                "input does not start with an ISA segment (input holds only 4 bytes of whitespace)"
            )
        );
        assert_eq!(
            read(b"\xEF\xBB\xBF\r\n").as_deref(),
            Some(
                "input does not start with an ISA segment (input holds only a UTF-8 byte order mark and 2 bytes of whitespace)"
            )
        );
        assert_eq!(
            read(b"\xEF\xBB\xBFGS*HP*X~").as_deref(),
            Some(
                "input does not start with an ISA segment (found bytes [47 53 2a 48 50 2a 58 7e] after skipping a UTF-8 byte order mark)"
            )
        );
        assert_eq!(
            read(b"").as_deref(),
            Some("input does not start with an ISA segment (input is empty)")
        );
        let mut input = b"\xEF\xBB\xBF\n".to_vec();
        input.extend_from_slice(ISA_5010);
        assert_eq!(
            Delimiters::from_isa_after_leading_trivia(&input),
            Delimiters::from_isa(ISA_5010)
        );
        assert_eq!(
            Delimiters::from_isa_after_leading_trivia(b"\nISA*00*"),
            Err(IsaError::Truncated {
                len: 7,
                separators_found: 2
            })
        );
    }

    #[test]
    fn truncated_displays_how_many_separators_were_found() {
        assert_eq!(
            IsaError::Truncated {
                len: 7,
                separators_found: 2
            }
            .to_string(),
            "ISA segment truncated after 7 bytes: found 2 of 16 element separators"
        );
        assert_eq!(
            IsaError::Truncated {
                len: 105,
                separators_found: 16
            }
            .to_string(),
            "ISA segment truncated after 105 bytes: ISA16 or the terminator is missing"
        );
    }

    #[test]
    fn is_special_covers_the_delimiters_the_tokenizer_splits_on() {
        let d = Delimiters::new(b'*', b':', b'~')
            .with_repetition(b'^')
            .with_release(b'?');
        for &byte in b"*:~?" {
            assert!(d.is_special(byte), "{}", byte as char);
        }
        assert!(!d.is_special(b'A'));
        assert!(
            !d.is_special(b'^'),
            "repetition is not split, so it is data"
        );
    }
}
