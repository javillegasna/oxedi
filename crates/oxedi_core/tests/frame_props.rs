//! Property: framing is lossless for *any* input, with or without a release byte.

mod common;

use common::delimiters;
use oxedi_core::{Delimiters, Frame, next_frame};
use proptest::prelude::*;

fn all_frames<'a>(mut input: &'a [u8], delims: &Delimiters) -> Vec<Frame<'a>> {
    let mut frames = Vec::new();
    while let Some((frame, rest)) = next_frame(input, delims) {
        frames.push(frame);
        input = rest;
    }
    frames
}

proptest! {
    /// Concatenating every frame's `raw` gives back the input, byte for byte.
    #[test]
    fn framing_is_lossless(
        input in prop::collection::vec(any::<u8>(), 0..256),
        use_release in any::<bool>(),
    ) {
        let frames = all_frames(&input, &delimiters(use_release));
        let rebuilt: Vec<u8> = frames.iter().flat_map(|f| f.raw.iter().copied()).collect();
        prop_assert_eq!(rebuilt, input);
    }

    /// Only the last frame may be unterminated, and terminated frames end in `~`.
    #[test]
    fn only_the_last_frame_may_be_unterminated(
        input in prop::collection::vec(any::<u8>(), 0..256),
        use_release in any::<bool>(),
    ) {
        let frames = all_frames(&input, &delimiters(use_release));
        for (i, frame) in frames.iter().enumerate() {
            if frame.terminated {
                prop_assert_eq!(frame.raw.last(), Some(&b'~'));
            } else {
                prop_assert_eq!(i, frames.len() - 1, "unterminated frame must be last");
            }
        }
    }
}
