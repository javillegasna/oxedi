//! Framing: find the next segment's bytes knowing only the delimiters.
//!
//! Pure and allocation-free. It does not know what a segment means; it only
//! knows where one ends. Every byte of the input is accounted for by exactly
//! one [`Frame::raw`], which is what makes the tokenizer lossless. A UTF-8
//! byte order mark at the very start of the input is leading trivia of the
//! first frame ([`first_frame`]); anywhere else it is data.

use crate::delimiters::Delimiters;

/// One segment's worth of bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame<'a> {
    /// Every byte this frame accounts for: leading trivia, body and terminator.
    /// Concatenating `raw` over all frames reproduces the input exactly.
    pub raw: &'a [u8],
    /// The segment text: `raw` without leading trivia and without the terminator.
    pub body: &'a [u8],
    /// `false` only for the last frame of an input that does not end with a terminator.
    pub terminated: bool,
}

/// Bytes tolerated between a terminator and the next segment.
pub const fn is_trivia(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
}

/// The UTF-8 byte order mark some editors write at the start of a file.
pub const BYTE_ORDER_MARK: &[u8] = b"\xEF\xBB\xBF";

/// How many bytes precede the first segment's text: a leading byte order
/// mark, then any trivia.
pub fn leading_trivia(input: &[u8]) -> usize {
    let skipped = if input.starts_with(BYTE_ORDER_MARK) {
        BYTE_ORDER_MARK.len()
    } else {
        0
    };
    let rest = input.get(skipped..).unwrap_or_default();
    skipped
        + rest
            .iter()
            .position(|&byte| !is_trivia(byte))
            .unwrap_or(rest.len())
}

/// Splits the first frame off the front of a whole input: as [`next_frame`],
/// except that a UTF-8 byte order mark at the very start is leading trivia,
/// kept in the frame's `raw` before the body.
pub fn first_frame<'a>(input: &'a [u8], delims: &Delimiters) -> Option<(Frame<'a>, &'a [u8])> {
    let Some(after) = input.strip_prefix(BYTE_ORDER_MARK) else {
        return next_frame(input, delims);
    };
    let Some((frame, rest)) = next_frame(after, delims) else {
        let rest = after.get(after.len()..).unwrap_or_default();
        let frame = Frame {
            raw: input,
            body: rest,
            terminated: false,
        };
        return Some((frame, rest));
    };
    let Some(raw) = input.get(..BYTE_ORDER_MARK.len() + frame.raw.len()) else {
        return next_frame(input, delims);
    };
    let frame = Frame {
        raw,
        body: frame.body,
        terminated: frame.terminated,
    };
    Some((frame, rest))
}

/// Splits the next frame off the front of `input`.
///
/// Returns the frame and the remaining input. Returns `None` only when `input`
/// is empty, so a loop over it always terminates: every call consumes at least
/// one byte.
pub fn next_frame<'a>(input: &'a [u8], delims: &Delimiters) -> Option<(Frame<'a>, &'a [u8])> {
    if input.is_empty() {
        return None;
    }
    let body_start = input
        .iter()
        .position(|&byte| !is_trivia(byte))
        .unwrap_or(input.len());
    match find_unescaped(&input[body_start..], delims.segment, delims.release) {
        Some(offset) => {
            let terminator_at = body_start + offset;
            let (raw, rest) = input.split_at(terminator_at + 1);
            let frame = Frame {
                raw,
                body: &input[body_start..terminator_at],
                terminated: true,
            };
            Some((frame, rest))
        }
        None => {
            let frame = Frame {
                raw: input,
                body: &input[body_start..],
                terminated: false,
            };
            Some((frame, &input[input.len()..]))
        }
    }
}

/// Index of the first `target` in `haystack` that is not escaped by `release`.
///
/// A release byte makes the byte after it literal, including another release
/// byte. A release byte at the very end escapes nothing and is never read past.
pub fn find_unescaped(haystack: &[u8], target: u8, release: Option<u8>) -> Option<usize> {
    let Some(release) = release else {
        return haystack.iter().position(|&byte| byte == target);
    };
    let mut at = 0;
    while let Some(&byte) = haystack.get(at) {
        if byte == release {
            at += 2;
        } else if byte == target {
            return Some(at);
        } else {
            at += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Delimiters;

    fn plain() -> Delimiters {
        Delimiters::new(b'*', b':', b'~')
    }

    fn with_release() -> Delimiters {
        plain().with_release(b'?')
    }

    #[test]
    fn splits_at_the_terminator() {
        let (frame, rest) = next_frame(b"ST*835~BPR~", &plain()).unwrap();
        assert_eq!(frame.raw, b"ST*835~");
        assert_eq!(frame.body, b"ST*835");
        assert!(frame.terminated);
        assert_eq!(rest, b"BPR~");
    }

    #[test]
    fn leading_trivia_belongs_to_raw_not_body() {
        let (frame, rest) = next_frame(b"\r\n GS*HP~", &plain()).unwrap();
        assert_eq!(frame.raw, b"\r\n GS*HP~");
        assert_eq!(frame.body, b"GS*HP");
        assert_eq!(rest, b"");
    }

    #[test]
    fn unterminated_tail_is_a_frame() {
        let (frame, rest) = next_frame(b"SE*5", &plain()).unwrap();
        assert_eq!(frame.raw, b"SE*5");
        assert_eq!(frame.body, b"SE*5");
        assert!(!frame.terminated);
        assert_eq!(rest, b"");
    }

    #[test]
    fn trivia_only_tail_is_a_frame_with_empty_body() {
        let (frame, rest) = next_frame(b"\n", &plain()).unwrap();
        assert_eq!(frame.raw, b"\n");
        assert_eq!(frame.body, b"");
        assert!(!frame.terminated);
        assert_eq!(rest, b"");
    }

    #[test]
    fn empty_segment_is_a_frame() {
        let (frame, rest) = next_frame(b"~SE~", &plain()).unwrap();
        assert_eq!(frame.raw, b"~");
        assert_eq!(frame.body, b"");
        assert!(frame.terminated);
        assert_eq!(rest, b"SE~");
    }

    #[test]
    fn release_makes_the_terminator_literal() {
        let (frame, rest) = next_frame(b"N1*A?~B~X~", &with_release()).unwrap();
        assert_eq!(frame.body, b"N1*A?~B");
        assert_eq!(rest, b"X~");
    }

    #[test]
    fn escaped_release_does_not_escape_the_terminator() {
        let (frame, _) = next_frame(b"N1*A??~B~", &with_release()).unwrap();
        assert_eq!(frame.body, b"N1*A??");
    }

    #[test]
    fn dangling_release_at_end_does_not_panic() {
        let (frame, rest) = next_frame(b"N1*A?", &with_release()).unwrap();
        assert_eq!(frame.body, b"N1*A?");
        assert!(!frame.terminated);
        assert_eq!(rest, b"");
    }

    #[test]
    fn a_leading_byte_order_mark_is_trivia_of_the_first_frame() {
        let (frame, rest) = first_frame(b"\xEF\xBB\xBF\r\nISA*00~GS~", &plain()).unwrap();
        assert_eq!(frame.raw, b"\xEF\xBB\xBF\r\nISA*00~");
        assert_eq!(frame.body, b"ISA*00");
        assert!(frame.terminated);
        assert_eq!(rest, b"GS~");
    }

    #[test]
    fn a_byte_order_mark_alone_is_a_trivia_only_frame() {
        let (frame, rest) = first_frame(b"\xEF\xBB\xBF", &plain()).unwrap();
        assert_eq!(frame.raw, b"\xEF\xBB\xBF");
        assert_eq!(frame.body, b"");
        assert!(!frame.terminated);
        assert_eq!(rest, b"");
    }

    #[test]
    fn without_a_byte_order_mark_the_first_frame_is_the_next_frame() {
        let input = b"\nST*835~SE~";
        assert_eq!(first_frame(input, &plain()), next_frame(input, &plain()));
        assert_eq!(first_frame(b"", &plain()), None);
    }

    #[test]
    fn a_byte_order_mark_after_the_first_frame_is_data() {
        let (frame, _) = next_frame(b"\xEF\xBB\xBFST~", &plain()).unwrap();
        assert_eq!(frame.body, b"\xEF\xBB\xBFST");
        let (frame, _) = first_frame(b"\xEF\xBB\xBF\xEF\xBB\xBFST~", &plain()).unwrap();
        assert_eq!(frame.body, b"\xEF\xBB\xBFST");
    }

    #[test]
    fn leading_trivia_counts_a_byte_order_mark_and_whitespace() {
        assert_eq!(leading_trivia(b"\xEF\xBB\xBF \r\nISA"), 6);
        assert_eq!(leading_trivia(b"\r\nISA"), 2);
        assert_eq!(leading_trivia(b"ISA"), 0);
        assert_eq!(leading_trivia(b" \xEF\xBB\xBFISA"), 1);
        assert_eq!(leading_trivia(b"\xEF\xBB\xBF"), 3);
        assert_eq!(leading_trivia(b""), 0);
    }

    #[test]
    fn empty_input_yields_no_frame() {
        assert_eq!(next_frame(b"", &plain()), None);
    }

    #[test]
    fn find_unescaped_without_release_is_plain_search() {
        assert_eq!(find_unescaped(b"ab~c", b'~', None), Some(2));
        assert_eq!(find_unescaped(b"abc", b'~', None), None);
    }

    #[test]
    fn find_unescaped_skips_the_byte_after_a_release() {
        assert_eq!(find_unescaped(b"a?~b~", b'~', Some(b'?')), Some(4));
        assert_eq!(find_unescaped(b"a?", b'~', Some(b'?')), None);
    }
}
