//! Framing: find the next segment's bytes knowing only the delimiters.
//!
//! Pure and allocation-free. It does not know what a segment means; it only
//! knows where one ends. Every byte of the input is accounted for by exactly
//! one [`Frame::raw`], which is what makes the tokenizer lossless.

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
