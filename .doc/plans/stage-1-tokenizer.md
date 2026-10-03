# Stage 1 — Framing + Tokenizer · Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn raw EDI bytes into a lazy, lossless stream of generic `Segment`s whose delimiters come from the file's ISA, with a symmetric writer for the reverse direction.

**Architecture:** Three pure layers, each ignorant of the one above: `frame` finds the next segment's bytes knowing only the delimiters; `element` splits a segment body into values honouring an optional release byte; `tokenizer` drives both as an `Iterator<Item = Segment<'a>>` that borrows from the input buffer. Every byte of input is accounted for by exactly one `Segment::raw`, so losslessness is a property of the construction, not a feature.

**Tech Stack:** Rust edition 2024 (stable), `proptest` 1 and `criterion` 0.5 (already dev-dependencies). No new dependencies.

**Spec:** `.doc/architectural-commitment.md` — §7 "Stage 1 · Framing + Tokenizer", argued from §2 (N1, N2, N4, N7), §3 (P1, P3, P4, P5, P7, P9) and §4.

> **All commands run from the project root** `/home/javillegasna/Desktop/org/personal/oxedi835/`.

## Global Constraints

- Edition 2024, `resolver = "3"`, stable toolchain pinned in `rust-toolchain.toml`.
- `[dependencies]` of `edi835_core` stays empty (P3: sans-IO, no runtime). Only `proptest` and `criterion` as dev-dependencies.
- Every command must pass `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo fmt --all -- --check`. Run both before every commit.
- Input is `&[u8]`, never `&str`. No UTF-8 assumption anywhere in the core.
- No `panic!`, `unwrap`, `expect` or indexing that can fail on arbitrary input in `src/`. Tests may unwrap.
- Delimiters are never assumed: they come from the ISA (counting separators, never fixed offsets) or from the caller.
- Every input byte ends up in exactly one `Segment::raw`. Trivia-only and empty frames are segments with an empty `id`, not errors.
- Fixtures under `crates/edi835_core/tests/fixtures/` are never modified.

## Review Focus

1. **A file with CRLF after every `~`** — the `\r\n` must land in the next segment's `raw` and the final one in a trailing empty-id segment, so concatenation is byte-exact. Test: `crlf_trivia_is_preserved` (Task 4).
2. **An ISA whose padding is wrong (105 or 102 bytes)** — delimiters must still be read correctly; a fixed-offset reader returns garbage. Test: `from_isa_counts_separators_on_short_isa` (Task 1).
3. **A value containing a byte that is a delimiter in *other* files** (`*` when the element separator is `|`) — must be kept as data. Test: `foreign_delimiter_byte_is_data` (Task 4).
4. **A file truncated in the middle of a segment** — the partial segment is emitted with `terminated: false`, never dropped, never a panic. Tests: `unterminated_tail_is_a_frame` (Task 2), `truncated_file_keeps_partial_segment` (Task 4).
5. **A release byte as the very last byte of the input or of a value** — must not panic, must not read past the end. Tests: `dangling_release_at_end_does_not_panic` (Task 2), `dangling_release_in_value_is_dropped` (Task 3).
6. **Non-UTF-8 bytes (Latin-1 names) inside values** — pass through untouched. Test: `non_utf8_bytes_pass_through` (Task 4).

---

## File Structure

```
crates/edi835_core/
├── Cargo.toml                       # [[bench]] renamed smoke → tokenize (Task 6)
├── src/
│   ├── lib.rs                       # module tree + re-exports; smoke test removed
│   ├── delimiters.rs                # Delimiters, IsaError, from_isa (counts separators)
│   ├── frame.rs                     # Frame, next_frame, find_unescaped, is_trivia (P5)
│   ├── element.rs                   # Element, Value (= Cow<[u8]>), split_raw, unescape
│   ├── segment.rs                   # Segment (parse from Frame), write_to, WriteError
│   └── tokenizer.rs                 # Tokenizer<'a>: Iterator<Item = Segment<'a>>
├── benches/
│   └── tokenize.rs                  # replaces smoke.rs: throughput over 3 fixtures
└── tests/
    ├── common/mod.rs                # unchanged helper + #![allow(dead_code)]
    ├── frame_props.rs               # proptest: framing is lossless (replaces proptest_harness.rs)
    ├── roundtrip_props.rs           # proptest: write_to ∘ tokenize = identity; fixtures rewrite byte-exact
    ├── tokenize_fixtures.rs         # N7 seam + N1 on the five fixtures
    ├── fixtures_harness.rs          # unchanged (Stage 0)
    └── fixtures/README.md           # documents each fixture's quirks (trizetto XX anomaly)
```

Each `src/` file has one job and depends only on the files above it in this list: `frame` uses `delimiters`; `element` uses `frame::find_unescaped`; `segment` uses `frame` + `element`; `tokenizer` uses all of them. Nothing knows what an 835 is.

---

## Task 1: `Delimiters` read from the ISA by counting separators

**Files:**
- Create: `crates/edi835_core/src/delimiters.rs`
- Modify: `crates/edi835_core/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub struct Delimiters { pub element: u8, pub component: u8, pub segment: u8, pub repetition: Option<u8>, pub release: Option<u8> }` (`Copy`, `Eq`, `Debug`)
  - `Delimiters::new(element: u8, component: u8, segment: u8) -> Self` (repetition and release `None`)
  - `Delimiters::with_repetition(self, u8) -> Self`, `Delimiters::with_release(self, u8) -> Self`
  - `Delimiters::from_isa(input: &[u8]) -> Result<Delimiters, IsaError>`
  - `pub enum IsaError { NotIsa, Truncated { len: usize } }` (`Copy`, `Eq`, `Debug`, `Display`, `Error`)

- [ ] **Step 1: Write the failing tests**

Create `crates/edi835_core/src/delimiters.rs` with only the test module for now:

```rust
//! Delimiters of an X12 interchange, read from the ISA segment (N2).

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
        assert_eq!(ISA_4010_SHORT.len(), 102, "fixture-derived ISA must be short");
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
        assert_eq!(Delimiters::from_isa(b"ISA"), Err(IsaError::Truncated { len: 3 }));
        assert_eq!(Delimiters::from_isa(b"ISA*00*"), Err(IsaError::Truncated { len: 7 }));
        let cut = &ISA_5010[..ISA_5010.len() - 1]; // terminator missing
        assert_eq!(Delimiters::from_isa(cut), Err(IsaError::Truncated { len: 105 }));
    }

    #[test]
    fn builders_set_optional_delimiters() {
        let d = Delimiters::new(b'|', b':', b'~').with_repetition(b'^').with_release(b'?');
        assert_eq!(d.element, b'|');
        assert_eq!(d.repetition, Some(b'^'));
        assert_eq!(d.release, Some(b'?'));
    }

    #[test]
    fn isa_error_displays_a_message() {
        assert_eq!(IsaError::NotIsa.to_string(), "input does not start with an ISA segment");
        assert_eq!(
            IsaError::Truncated { len: 7 }.to_string(),
            "ISA segment truncated after 7 bytes"
        );
    }
}
```

Replace the whole of `crates/edi835_core/src/lib.rs` with:

```rust
//! `edi835_core` — lossless, fast, data-driven EDI 835 parser core.
//!
//! Stage 1: framing + tokenizer. Bytes in, a lazy stream of generic segments
//! out. Nothing in this crate knows what an 835 is (P1); see
//! `.doc/architectural-commitment.md`.

pub mod delimiters;

pub use delimiters::{Delimiters, IsaError};
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p edi835_core --lib`
Expected: compile error — `cannot find type Delimiters` / `IsaError` in this scope.

- [ ] **Step 3: Implement `Delimiters` and `from_isa`**

Insert above the `#[cfg(test)]` module in `crates/edi835_core/src/delimiters.rs`:

```rust
//! Delimiters of an X12 interchange, read from the ISA segment (N2).
//!
//! The ISA is nominally 106 bytes of fixed width, but real payer files pad
//! ISA06/ISA08 wrongly (two of our fixtures are 105 and 102 bytes long). So we
//! never trust byte offsets: we count element separators. The separator right
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
        Self { element, component, segment, repetition: None, release: None }
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
            .filter(|(_, &byte)| byte == element)
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

        Ok(Self { element, component, segment, repetition, release: None })
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p edi835_core --lib`
Expected: `test result: ok. 8 passed` (the eight `delimiters::tests::*`), no warnings.

- [ ] **Step 5: Lint, format, commit**

Run: `cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all && cargo fmt --all -- --check`
Expected: clean, exit 0.

```bash
git add crates/edi835_core/src/delimiters.rs crates/edi835_core/src/lib.rs
git commit -m "feat(delimiters): read ISA delimiters by counting separators"
```

---

## Task 2: Framing — `next_frame` splits input into lossless frames

**Files:**
- Create: `crates/edi835_core/src/frame.rs`
- Create: `crates/edi835_core/tests/frame_props.rs`
- Delete: `crates/edi835_core/tests/proptest_harness.rs`
- Modify: `crates/edi835_core/src/lib.rs`

**Interfaces:**
- Consumes: `Delimiters` (Task 1).
- Produces:
  - `pub struct Frame<'a> { pub raw: &'a [u8], pub body: &'a [u8], pub terminated: bool }` (`Copy`, `Eq`, `Debug`)
  - `pub fn next_frame<'a>(input: &'a [u8], delims: &Delimiters) -> Option<(Frame<'a>, &'a [u8])>`
  - `pub fn find_unescaped(haystack: &[u8], target: u8, release: Option<u8>) -> Option<usize>`
  - `pub const fn is_trivia(byte: u8) -> bool`

- [ ] **Step 1: Write the failing unit tests**

Create `crates/edi835_core/src/frame.rs` with only:

```rust
//! Framing (P5): find the next segment's bytes knowing only the delimiters.

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
```

Add to `crates/edi835_core/src/lib.rs`:

```rust
pub mod frame;

pub use frame::{Frame, next_frame};
```

(keep the existing `pub mod delimiters;` and its re-export; the `pub mod` lines go together, then the `pub use` lines.)

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p edi835_core --lib frame`
Expected: compile error — `cannot find function next_frame` / `find_unescaped`.

- [ ] **Step 3: Implement framing**

Insert above the test module in `crates/edi835_core/src/frame.rs`:

```rust
//! Framing (P5): find the next segment's bytes knowing only the delimiters.
//!
//! Pure and allocation-free. It does not know what a segment means; it only
//! knows where one ends. Every byte of the input is accounted for by exactly
//! one [`Frame::raw`], which is what makes the tokenizer lossless (N1).

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
    let body_start = input.iter().position(|&byte| !is_trivia(byte)).unwrap_or(input.len());
    match find_unescaped(&input[body_start..], delims.segment, delims.release) {
        Some(offset) => {
            let terminator_at = body_start + offset;
            let (raw, rest) = input.split_at(terminator_at + 1);
            let frame = Frame { raw, body: &input[body_start..terminator_at], terminated: true };
            Some((frame, rest))
        }
        None => {
            let frame = Frame { raw: input, body: &input[body_start..], terminated: false };
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
```

- [ ] **Step 4: Run the unit tests to verify they pass**

Run: `cargo test -p edi835_core --lib frame`
Expected: `test result: ok. 11 passed`.

- [ ] **Step 5: Write the failing property test**

Delete the placeholder: `git rm crates/edi835_core/tests/proptest_harness.rs`.

Create `crates/edi835_core/tests/frame_props.rs`:

```rust
//! Property: framing is lossless for *any* input, with or without a release byte.

use edi835_core::{Delimiters, Frame, next_frame};
use proptest::prelude::*;

fn all_frames<'a>(mut input: &'a [u8], delims: &Delimiters) -> Vec<Frame<'a>> {
    let mut frames = Vec::new();
    while let Some((frame, rest)) = next_frame(input, delims) {
        frames.push(frame);
        input = rest;
    }
    frames
}

fn delimiters(use_release: bool) -> Delimiters {
    let delims = Delimiters::new(b'*', b':', b'~');
    if use_release { delims.with_release(b'?') } else { delims }
}

proptest! {
    /// Concatenating every frame's `raw` gives back the input, byte for byte (N1).
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
```

- [ ] **Step 6: Run the property tests and watch them pass against the fresh implementation**

Run: `cargo test -p edi835_core --test frame_props`
Expected: `test result: ok. 2 passed`. (These properties are the gate for the code written in Step 3; if either fails, proptest prints a minimal counterexample — fix `next_frame`, never the property.)

- [ ] **Step 7: Lint, format, commit**

Run: `cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all && cargo fmt --all -- --check && cargo test --workspace --locked`
Expected: all clean; full suite green (delimiters 8 + frame 11 + frame_props 2 + fixtures_harness 1 + smoke 0 — the lib.rs smoke test was removed in Task 1).

```bash
git add -A crates/edi835_core
git commit -m "feat(frame): lossless framing with optional release byte"
```

---

## Task 3: Elements — split values honouring the release byte, borrow when possible

**Files:**
- Create: `crates/edi835_core/src/element.rs`
- Modify: `crates/edi835_core/src/lib.rs`

**Interfaces:**
- Consumes: `frame::find_unescaped` (Task 2).
- Produces:
  - `pub type Value<'a> = Cow<'a, [u8]>`
  - `pub enum Element<'a> { Simple(Value<'a>), Composite(Vec<Value<'a>>) }` (`Clone`, `Eq`, `Debug`)
  - `Element::parse(raw: &'a [u8], component: u8, release: Option<u8>) -> Element<'a>`
  - `Element::simple(&self) -> Option<&[u8]>`
  - `pub fn split_raw<'a>(raw: &'a [u8], sep: u8, release: Option<u8>) -> Vec<&'a [u8]>`
  - `pub fn unescape(raw: &[u8], release: Option<u8>) -> Value<'_>`

- [ ] **Step 1: Write the failing tests**

Create `crates/edi835_core/src/element.rs` with only:

```rust
//! Element values of a segment.

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn simple_value_borrows_from_the_input() {
        let element = Element::parse(b"835", b':', None);
        assert_eq!(element, Element::Simple(Cow::Borrowed(b"835")));
        assert!(matches!(element, Element::Simple(Cow::Borrowed(_))));
    }

    #[test]
    fn composite_splits_on_the_component_separator() {
        let element = Element::parse(b"HC:99213", b':', None);
        assert_eq!(
            element,
            Element::Composite(vec![Cow::Borrowed(b"HC"), Cow::Borrowed(b"99213")])
        );
    }

    #[test]
    fn composite_keeps_empty_components() {
        let element = Element::parse(b"HC::X", b':', None);
        assert_eq!(
            element,
            Element::Composite(vec![Cow::Borrowed(b"HC"), Cow::Borrowed(b""), Cow::Borrowed(b"X")])
        );
    }

    #[test]
    fn release_makes_the_component_separator_literal_and_owns_the_value() {
        let element = Element::parse(b"A?:B", b':', Some(b'?'));
        assert_eq!(element, Element::Simple(Cow::Owned(b"A:B".to_vec())));
        assert!(matches!(element, Element::Simple(Cow::Owned(_))));
    }

    #[test]
    fn release_configured_but_absent_still_borrows() {
        let element = Element::parse(b"ABC", b':', Some(b'?'));
        assert!(matches!(element, Element::Simple(Cow::Borrowed(_))));
    }

    #[test]
    fn escaped_release_is_a_literal_release_byte() {
        assert_eq!(unescape(b"A??B", Some(b'?')), Cow::<[u8]>::Owned(b"A?B".to_vec()));
    }

    #[test]
    fn dangling_release_in_value_is_dropped() {
        assert_eq!(unescape(b"AB?", Some(b'?')), Cow::<[u8]>::Owned(b"AB".to_vec()));
    }

    #[test]
    fn split_raw_keeps_empty_pieces_and_never_returns_none() {
        assert_eq!(split_raw(b"a::b", b':', None), vec![&b"a"[..], b"", b"b"]);
        assert_eq!(split_raw(b"", b':', None), vec![&b""[..]]);
    }

    #[test]
    fn simple_accessor_returns_none_for_composites() {
        assert_eq!(Element::parse(b"X", b':', None).simple(), Some(&b"X"[..]));
        assert_eq!(Element::parse(b"X:Y", b':', None).simple(), None);
    }
}
```

Add to `crates/edi835_core/src/lib.rs` (next to the other `pub mod` / `pub use` lines):

```rust
pub mod element;

pub use element::{Element, Value};
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p edi835_core --lib element`
Expected: compile error — `cannot find type Element`.

- [ ] **Step 3: Implement elements**

Insert above the test module in `crates/edi835_core/src/element.rs`:

```rust
//! Element values of a segment.
//!
//! Splitting honours an optional release byte. Values borrow from the input
//! (N4) unless a release byte had to be removed, in which case they are owned:
//! that is exactly what `Cow` is for, and it is the first taste of D1.

use std::borrow::Cow;

use crate::frame::find_unescaped;

/// A value: borrowed from the buffer, or owned after unescaping.
pub type Value<'a> = Cow<'a, [u8]>;

/// One element of a segment: simple, or composite when it contains an
/// unescaped component separator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Element<'a> {
    /// A single value.
    Simple(Value<'a>),
    /// Values separated by the component separator, in order.
    Composite(Vec<Value<'a>>),
}

impl<'a> Element<'a> {
    /// Parses one raw element (the bytes between two element separators).
    pub fn parse(raw: &'a [u8], component: u8, release: Option<u8>) -> Self {
        if find_unescaped(raw, component, release).is_some() {
            let values = split_raw(raw, component, release)
                .into_iter()
                .map(|piece| unescape(piece, release))
                .collect();
            Element::Composite(values)
        } else {
            Element::Simple(unescape(raw, release))
        }
    }

    /// The value of a simple element; `None` for a composite.
    pub fn simple(&self) -> Option<&[u8]> {
        match self {
            Element::Simple(value) => Some(value),
            Element::Composite(_) => None,
        }
    }
}

/// Splits `raw` on every unescaped `sep`. Pieces are not unescaped. Always
/// returns at least one piece, so the caller can take the first as an id.
pub fn split_raw<'a>(mut raw: &'a [u8], sep: u8, release: Option<u8>) -> Vec<&'a [u8]> {
    let mut pieces = Vec::new();
    while let Some(at) = find_unescaped(raw, sep, release) {
        pieces.push(&raw[..at]);
        raw = &raw[at + 1..];
    }
    pieces.push(raw);
    pieces
}

/// Removes release bytes, keeping the byte each one protects. Borrows when
/// there is nothing to remove. A release byte at the very end protects nothing
/// and is dropped.
pub fn unescape(raw: &[u8], release: Option<u8>) -> Value<'_> {
    match release {
        Some(release) if raw.contains(&release) => {
            let mut out = Vec::with_capacity(raw.len());
            let mut literal_next = false;
            for &byte in raw {
                if literal_next {
                    out.push(byte);
                    literal_next = false;
                } else if byte == release {
                    literal_next = true;
                } else {
                    out.push(byte);
                }
            }
            Cow::Owned(out)
        }
        _ => Cow::Borrowed(raw),
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p edi835_core --lib element`
Expected: `test result: ok. 9 passed`.

- [ ] **Step 5: Lint, format, commit**

Run: `cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all && cargo fmt --all -- --check`
Expected: clean.

```bash
git add crates/edi835_core/src/element.rs crates/edi835_core/src/lib.rs
git commit -m "feat(element): split values with optional release byte, borrow via Cow"
```

---

## Task 4: `Segment` and the `Tokenizer` iterator, proven on the five fixtures

**Files:**
- Create: `crates/edi835_core/src/segment.rs`
- Create: `crates/edi835_core/src/tokenizer.rs`
- Create: `crates/edi835_core/tests/tokenize_fixtures.rs`
- Create: `crates/edi835_core/tests/fixtures/README.md`
- Modify: `crates/edi835_core/src/lib.rs`
- Modify: `crates/edi835_core/tests/common/mod.rs` (add `#![allow(dead_code)]`)

**Interfaces:**
- Consumes: `Delimiters`, `IsaError` (Task 1); `Frame`, `next_frame`, `is_trivia` (Task 2); `Element`, `split_raw` (Task 3); `common::load_fixture(name) -> Vec<u8>` (Stage 0).
- Produces:
  - `pub struct Segment<'a> { pub index: usize, pub raw: &'a [u8], pub id: &'a [u8], pub elements: Vec<Element<'a>>, pub terminated: bool }` (`Clone`, `Eq`, `Debug`)
  - `Segment::parse(index: usize, frame: Frame<'a>, delims: &Delimiters) -> Segment<'a>`
  - `Segment::is_empty(&self) -> bool` (empty id and no elements)
  - `Segment::element(&self, position: usize) -> Option<&Element<'a>>` (1-based, X12 style: `element(1)` is `XX01`)
  - `pub struct Tokenizer<'a>` implementing `Iterator<Item = Segment<'a>>`
  - `Tokenizer::new(input: &'a [u8]) -> Result<Tokenizer<'a>, IsaError>`
  - `Tokenizer::with_delimiters(input: &'a [u8], delims: Delimiters) -> Tokenizer<'a>`
  - `Tokenizer::delimiters(&self) -> &Delimiters`

- [ ] **Step 1: Write the failing unit tests for `Segment`**

Create `crates/edi835_core/src/segment.rs` with only:

```rust
//! A generic segment, borrowed from the input buffer.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Delimiters, Frame};
    use std::borrow::Cow;

    fn frame(body: &[u8]) -> Frame<'_> {
        Frame { raw: body, body, terminated: true }
    }

    #[test]
    fn parse_splits_id_and_elements() {
        let delims = Delimiters::new(b'*', b':', b'~');
        let segment = Segment::parse(7, frame(b"ST*835*1234"), &delims);
        assert_eq!(segment.index, 7);
        assert_eq!(segment.id, b"ST");
        assert_eq!(
            segment.elements,
            vec![Element::Simple(Cow::Borrowed(b"835")), Element::Simple(Cow::Borrowed(b"1234"))]
        );
        assert!(segment.terminated);
    }

    #[test]
    fn parse_of_empty_body_is_an_empty_segment() {
        let delims = Delimiters::new(b'*', b':', b'~');
        let segment = Segment::parse(0, frame(b""), &delims);
        assert_eq!(segment.id, b"");
        assert!(segment.elements.is_empty());
        assert!(segment.is_empty());
    }

    #[test]
    fn parse_keeps_trailing_empty_elements() {
        let delims = Delimiters::new(b'*', b':', b'~');
        let segment = Segment::parse(0, frame(b"BPR*I**C"), &delims);
        assert_eq!(segment.elements.len(), 3);
        assert_eq!(segment.element(2).and_then(Element::simple), Some(&b""[..]));
    }

    #[test]
    fn element_is_one_based_like_x12() {
        let delims = Delimiters::new(b'*', b':', b'~');
        let segment = Segment::parse(0, frame(b"CLP*123*1"), &delims);
        assert_eq!(segment.element(1).and_then(Element::simple), Some(&b"123"[..]));
        assert_eq!(segment.element(2).and_then(Element::simple), Some(&b"1"[..]));
        assert_eq!(segment.element(0), None);
        assert_eq!(segment.element(3), None);
    }
}
```

- [ ] **Step 2: Write the failing unit tests for `Tokenizer`**

Create `crates/edi835_core/src/tokenizer.rs` with only:

```rust
//! The tokenizer: a lazy iterator of segments over a byte buffer.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Delimiters, Element, IsaError};
    use std::borrow::Cow;

    const ISA: &[u8] =
        b"ISA*00*          *00*          *ZZ*EMEDNYBAT      *ZZ*ETIN           *100101*1000*^*00501*006000600*0*T*:~";

    fn plain() -> Delimiters {
        Delimiters::new(b'*', b':', b'~')
    }

    fn concat_raw(segments: &[Segment<'_>]) -> Vec<u8> {
        segments.iter().flat_map(|s| s.raw.iter().copied()).collect()
    }

    #[test]
    fn with_delimiters_yields_segments_in_order_with_consecutive_indices() {
        let segments: Vec<_> = Tokenizer::with_delimiters(b"ST*835*1~SE*2*1~", plain()).collect();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].id, b"ST");
        assert_eq!(segments[1].id, b"SE");
        assert_eq!(segments[0].index, 0);
        assert_eq!(segments[1].index, 1);
        assert_eq!(
            segments[0].elements,
            vec![Element::Simple(Cow::Borrowed(b"835")), Element::Simple(Cow::Borrowed(b"1"))]
        );
    }

    #[test]
    fn trailing_newline_becomes_an_empty_unterminated_segment() {
        let input = b"ST*835~\n";
        let segments: Vec<_> = Tokenizer::with_delimiters(input, plain()).collect();
        assert_eq!(segments.len(), 2);
        assert!(segments[1].is_empty());
        assert!(!segments[1].terminated);
        assert_eq!(segments[1].raw, b"\n");
        assert_eq!(concat_raw(&segments), input);
    }

    #[test]
    fn crlf_trivia_is_preserved() {
        let input = b"ST*835~\r\nSE*2*1~\r\n";
        let segments: Vec<_> = Tokenizer::with_delimiters(input, plain()).collect();
        let ids: Vec<&[u8]> = segments.iter().map(|s| s.id).collect();
        assert_eq!(ids, vec![&b"ST"[..], b"SE", b""]);
        assert_eq!(segments[1].raw, b"\r\nSE*2*1~");
        assert_eq!(concat_raw(&segments), input);
    }

    #[test]
    fn empty_segment_is_preserved_in_place() {
        let segments: Vec<_> = Tokenizer::with_delimiters(b"ST~~SE~", plain()).collect();
        assert_eq!(segments.len(), 3);
        assert!(segments[1].is_empty());
        assert!(segments[1].terminated);
        assert_eq!(segments[1].raw, b"~");
    }

    #[test]
    fn truncated_file_keeps_partial_segment() {
        let segments: Vec<_> = Tokenizer::with_delimiters(b"ST*835~SE*2", plain()).collect();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[1].id, b"SE");
        assert!(!segments[1].terminated);
    }

    #[test]
    fn foreign_delimiter_byte_is_data() {
        let delims = Delimiters::new(b'|', b':', b'~');
        let segments: Vec<_> = Tokenizer::with_delimiters(b"REF|F2|LC*A438D~", delims).collect();
        assert_eq!(segments[0].element(2).and_then(Element::simple), Some(&b"LC*A438D"[..]));
    }

    #[test]
    fn non_utf8_bytes_pass_through() {
        let segments: Vec<_> = Tokenizer::with_delimiters(b"NM1*QC*1*P\xC9REZ~", plain()).collect();
        assert_eq!(segments[0].element(3).and_then(Element::simple), Some(&b"P\xC9REZ"[..]));
    }

    #[test]
    fn composite_element_inside_a_segment() {
        let segments: Vec<_> = Tokenizer::with_delimiters(b"SVC*HC:99213*100~", plain()).collect();
        assert_eq!(
            segments[0].element(1),
            Some(&Element::Composite(vec![Cow::Borrowed(b"HC"), Cow::Borrowed(b"99213")]))
        );
    }

    #[test]
    fn new_reads_delimiters_from_the_isa() {
        let mut input = ISA.to_vec();
        input.extend_from_slice(b"GS*HP:X~");
        let tokenizer = Tokenizer::new(&input).unwrap();
        assert_eq!(tokenizer.delimiters().component, b':');
        let segments: Vec<_> = tokenizer.collect();
        assert_eq!(segments[0].id, b"ISA");
        assert_eq!(
            segments[1].element(1),
            Some(&Element::Composite(vec![Cow::Borrowed(b"HP"), Cow::Borrowed(b"X")]))
        );
    }

    #[test]
    fn new_tolerates_trivia_before_the_isa() {
        let mut input = b"\r\n".to_vec();
        input.extend_from_slice(ISA);
        let segments: Vec<_> = Tokenizer::new(&input).unwrap().collect();
        assert_eq!(segments[0].id, b"ISA");
        assert!(segments[0].raw.starts_with(b"\r\n"));
    }

    #[test]
    fn new_fails_without_an_isa() {
        assert_eq!(Tokenizer::new(b"ST*835~").err(), Some(IsaError::NotIsa));
        assert_eq!(Tokenizer::new(b"").err(), Some(IsaError::NotIsa));
    }

    #[test]
    fn release_can_be_injected_through_with_delimiters() {
        let delims = Delimiters::from_isa(ISA).unwrap().with_release(b'?');
        let mut input = ISA.to_vec();
        input.extend_from_slice(b"N1*PR*A?*B~");
        let segments: Vec<_> = Tokenizer::with_delimiters(&input, delims).collect();
        assert_eq!(segments[1].element(2).and_then(Element::simple), Some(&b"A*B"[..]));
    }
}
```

Add to `crates/edi835_core/src/lib.rs`:

```rust
pub mod segment;
pub mod tokenizer;

pub use segment::Segment;
pub use tokenizer::Tokenizer;
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p edi835_core --lib`
Expected: compile error — `cannot find type Segment` / `Tokenizer`.

- [ ] **Step 4: Implement `Segment`**

Insert above the test module in `crates/edi835_core/src/segment.rs`:

```rust
//! A generic segment, borrowed from the input buffer.
//!
//! A segment is an id plus elements. It carries its own `raw` bytes and its
//! position in the stream so that every later layer can point back at the
//! exact input it came from (N1).

use crate::delimiters::Delimiters;
use crate::element::{Element, split_raw};
use crate::frame::Frame;

/// One segment of the input. Everything borrows from the buffer except
/// values that needed unescaping (see [`Element`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment<'a> {
    /// 0-based position in the token stream.
    pub index: usize,
    /// Exact bytes this segment accounts for: leading trivia, body, terminator.
    pub raw: &'a [u8],
    /// Segment identifier (`ISA`, `CLP`, …). Empty for an empty or trivia-only frame.
    pub id: &'a [u8],
    /// Elements after the identifier, in X12 order: `elements[0]` is `XX01`.
    pub elements: Vec<Element<'a>>,
    /// `false` only when the input ended before this segment's terminator.
    pub terminated: bool,
}

impl<'a> Segment<'a> {
    /// Parses a frame's body into id and elements.
    pub fn parse(index: usize, frame: Frame<'a>, delims: &Delimiters) -> Self {
        let mut pieces = split_raw(frame.body, delims.element, delims.release).into_iter();
        let id = pieces.next().unwrap_or_default();
        let elements = pieces
            .map(|piece| Element::parse(piece, delims.component, delims.release))
            .collect();
        Self { index, raw: frame.raw, id, elements, terminated: frame.terminated }
    }

    /// `true` when the frame had no content: `~~`, or trailing trivia.
    pub fn is_empty(&self) -> bool {
        self.id.is_empty() && self.elements.is_empty()
    }

    /// Element by its 1-based X12 position: `element(1)` is `XX01`.
    pub fn element(&self, position: usize) -> Option<&Element<'a>> {
        self.elements.get(position.checked_sub(1)?)
    }
}
```

- [ ] **Step 5: Implement `Tokenizer`**

Insert above the test module in `crates/edi835_core/src/tokenizer.rs`:

```rust
//! The tokenizer: a lazy iterator of segments over a byte buffer (P4).
//!
//! It has no per-segment errors. Every frame becomes a [`Segment`], including
//! empty ones (`~~`) and the file's trailing trivia, which carry an empty `id`.
//! N1 and P7 hold by construction: nothing is dropped, nothing panics, and the
//! consumer decides what an empty id means. Between two calls to `next` the
//! tokenizer is simply paused.

use crate::delimiters::{Delimiters, IsaError};
use crate::frame::{is_trivia, next_frame};
use crate::segment::Segment;

/// Iterator of segments over `input`.
#[derive(Debug, Clone)]
pub struct Tokenizer<'a> {
    rest: &'a [u8],
    delims: Delimiters,
    next_index: usize,
}

impl<'a> Tokenizer<'a> {
    /// Reads the delimiters from the ISA segment, which may be preceded by trivia.
    ///
    /// `release` is never read from the file. To use one, read the delimiters
    /// with [`Delimiters::from_isa`], add it, and call [`Tokenizer::with_delimiters`].
    pub fn new(input: &'a [u8]) -> Result<Self, IsaError> {
        let start = input.iter().position(|&byte| !is_trivia(byte)).unwrap_or(input.len());
        let delims = Delimiters::from_isa(&input[start..])?;
        Ok(Self::with_delimiters(input, delims))
    }

    /// Tokenizes with caller-supplied delimiters: fragments without an ISA, or
    /// an ISA-derived set extended with a release byte.
    pub const fn with_delimiters(input: &'a [u8], delims: Delimiters) -> Self {
        Self { rest: input, delims, next_index: 0 }
    }

    /// The delimiters in use.
    pub const fn delimiters(&self) -> &Delimiters {
        &self.delims
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let (frame, rest) = next_frame(self.rest, &self.delims)?;
        self.rest = rest;
        let segment = Segment::parse(self.next_index, frame, &self.delims);
        self.next_index += 1;
        Some(segment)
    }
}
```

- [ ] **Step 6: Run the unit tests to verify they pass**

Run: `cargo test -p edi835_core --lib`
Expected: `test result: ok. 44 passed` (delimiters 8 + frame 11 + element 9 + segment 4 + tokenizer 12).

- [ ] **Step 7: Write the failing fixture integration test (N7 seam + N1 on real files)**

Prepend to `crates/edi835_core/tests/common/mod.rs` (first line of the file, before the `//!` doc comment):

```rust
#![allow(dead_code)] // each test binary compiles this module; not all use every helper
```

Create `crates/edi835_core/tests/tokenize_fixtures.rs`:

```rust
//! Stage 1 gate on the five real fixtures: lossless by construction (N1), the
//! framing→tokenizer seam (N7), and the ISA-less fragment case.

mod common;

use edi835_core::{Delimiters, IsaError, Segment, Tokenizer, next_frame};

/// (fixture, number of `~` in it). Terminated segments must match exactly.
const ENVELOPED: &[(&str, usize)] = &[
    ("emedny_sample.txt", 69),
    ("united_healthcare_legacy_sample.txt", 65),
    ("multi_claim_sample.txt", 51),
    ("trizetto_sample.rmt", 22),
];
const FRAGMENT: (&str, usize) = ("blue_cross_nc_sample.txt", 32);

fn concat_raw(segments: &[Segment<'_>]) -> Vec<u8> {
    segments.iter().flat_map(|s| s.raw.iter().copied()).collect()
}

fn count_frames(mut input: &[u8], delims: &Delimiters) -> usize {
    let mut n = 0;
    while let Some((_, rest)) = next_frame(input, delims) {
        n += 1;
        input = rest;
    }
    n
}

fn assert_stage1_gate(bytes: &[u8], segments: &[Segment<'_>], delims: &Delimiters, tildes: usize) {
    assert_eq!(concat_raw(segments), bytes, "concatenated raw must equal the file (N1)");
    assert_eq!(segments.iter().filter(|s| s.terminated).count(), tildes);
    assert_eq!(bytes.iter().filter(|&&b| b == b'~').count(), tildes, "fixture changed?");
    assert_eq!(segments.len(), count_frames(bytes, delims), "tokenizer must emit one segment per frame (N7)");
    for (expected, segment) in segments.iter().enumerate() {
        assert_eq!(segment.index, expected, "indices must be consecutive from 0");
    }
    for segment in &segments[..segments.len() - 1] {
        assert!(segment.terminated, "only the last segment may be unterminated");
    }
    assert!(segments.iter().filter(|s| s.terminated).all(|s| !s.id.is_empty()), "no `~~` in fixtures");
}

#[test]
fn enveloped_fixtures_tokenize_losslessly_from_their_isa() {
    for &(name, tildes) in ENVELOPED {
        let bytes = common::load_fixture(name);
        let tokenizer = Tokenizer::new(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        let delims = *tokenizer.delimiters();
        let segments: Vec<_> = tokenizer.collect();
        assert_eq!(segments[0].id, b"ISA", "{name}");
        assert_stage1_gate(&bytes, &segments, &delims, tildes);
    }
}

#[test]
fn fixtures_with_a_newline_per_segment_end_in_an_empty_segment() {
    for name in ["multi_claim_sample.txt", "trizetto_sample.rmt"] {
        let bytes = common::load_fixture(name);
        let last = Tokenizer::new(&bytes).unwrap().last().unwrap();
        assert!(last.is_empty(), "{name}: trailing LF must be its own empty segment");
        assert_eq!(last.raw, b"\n", "{name}");
    }
}

#[test]
fn fragment_without_isa_needs_caller_delimiters() {
    let (name, tildes) = FRAGMENT;
    let bytes = common::load_fixture(name);
    assert_eq!(Tokenizer::new(&bytes).err(), Some(IsaError::NotIsa));
    let delims = Delimiters::new(b'*', b':', b'~');
    let segments: Vec<_> = Tokenizer::with_delimiters(&bytes, delims).collect();
    assert_eq!(segments[0].id, b"ST");
    assert_stage1_gate(&bytes, &segments, &delims, tildes);
}

#[test]
fn trizetto_anomaly_is_preserved_as_an_unknown_segment() {
    // The POC fixture has `~XX*654321~` where `*` was probably intended. It stays
    // byte-exact: a lossless tokenizer keeps it as a segment with id `XX` (N1).
    let bytes = common::load_fixture("trizetto_sample.rmt");
    let ids: Vec<Vec<u8>> = Tokenizer::new(&bytes).unwrap().map(|s| s.id.to_vec()).collect();
    assert!(ids.contains(&b"XX".to_vec()));
}

#[test]
fn delimiters_read_from_each_fixture_match_the_known_values() {
    let expect = [
        ("emedny_sample.txt", b':', Some(b'^')),
        ("united_healthcare_legacy_sample.txt", b'>', Some(b'^')),
        ("multi_claim_sample.txt", b'>', None),
        ("trizetto_sample.rmt", b'>', None),
    ];
    for (name, component, repetition) in expect {
        let bytes = common::load_fixture(name);
        let d = Delimiters::from_isa(&bytes).unwrap();
        assert_eq!((d.element, d.component, d.segment), (b'*', component, b'~'), "{name}");
        assert_eq!(d.repetition, repetition, "{name}");
    }
}
```

- [ ] **Step 8: Run the fixture test and compare against the expected counts**

Run: `cargo test -p edi835_core --test tokenize_fixtures`
Expected: `test result: ok. 5 passed`. If a count assertion fails, re-measure with `tr -cd '~' < crates/edi835_core/tests/fixtures/<name> | wc -c` before touching code: the numbers in `ENVELOPED`/`FRAGMENT` were measured on 2026-10-02 and the fixtures must not have changed.

- [ ] **Step 9: Document the fixtures**

Create `crates/edi835_core/tests/fixtures/README.md`:

```markdown
# Fixtures

Real-shaped 835 files inherited byte-for-byte from the `fast_edi835` POC. **Never edit
them**: they are the oracle for the lossless gates (N1). Counts measured 2026-10-02.

| File | ISA | Version | Component | Repetition | `~` | Line endings |
|------|-----|---------|-----------|------------|-----|--------------|
| `emedny_sample.txt` | 106 B | 00501 | `:` | `^` | 69 | none |
| `united_healthcare_legacy_sample.txt` | 106 B | 00501 | `>` | `^` | 65 | none |
| `multi_claim_sample.txt` | **105 B** (ISA06 padded to 14) | 00401 | `>` | — | 51 | LF after every `~` |
| `trizetto_sample.rmt` | **102 B** (ISA06/08 padded to 13) | 00401 | `>` | — | 22 | LF after every `~` |
| `blue_cross_nc_sample.txt` | **none** (starts at `ST`) | — | caller-supplied | — | 32 | none |

Known quirks, kept on purpose:

- `trizetto_sample.rmt` line 7: `N1*PR*INSURANCE COMPANY OF AMERICA~XX*654321~` — a `~` where
  `*` was almost certainly meant. It yields a bogus `XX` segment and the `SE` count does not
  match. It is our standing case for "unknown segment preserved" (N1) and "malformed input
  does not abort" (P7).
- The two short ISAs are why delimiters are read by counting separators, never by offset.
- `blue_cross_nc_sample.txt` is a fragment without an envelope: `Tokenizer::new` must fail
  with `NotIsa` and `Tokenizer::with_delimiters` must work.
```

- [ ] **Step 10: Lint, format, full suite, commit**

Run: `cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all && cargo fmt --all -- --check && cargo test --workspace --locked`
Expected: clean; suite green (lib 44 + frame_props 2 + tokenize_fixtures 5 + fixtures_harness 1).

```bash
git add -A crates/edi835_core
git commit -m "feat(tokenizer): lazy lossless Segment iterator, proven on all fixtures"
```

---

## Task 5: Symmetric writer — `Segment::write_to` and the round-trip property

**Files:**
- Modify: `crates/edi835_core/src/delimiters.rs` (add `is_special`)
- Modify: `crates/edi835_core/src/segment.rs` (add `WriteError`, `write_to`)
- Modify: `crates/edi835_core/src/lib.rs` (re-export `WriteError`)
- Create: `crates/edi835_core/tests/roundtrip_props.rs`

**Interfaces:**
- Consumes: `Segment`, `Element`, `Tokenizer`, `Delimiters` (Tasks 1–4).
- Produces:
  - `Delimiters::is_special(&self, byte: u8) -> bool` (true for element, component, segment, repetition, release)
  - `pub enum WriteError { Io(std::io::Error), DelimiterInValue { byte: u8 } }` (`Debug`, `Display`, `Error`, `From<io::Error>`)
  - `Segment::write_to<W: std::io::Write>(&self, delims: &Delimiters, out: &mut W) -> Result<(), WriteError>`

- [ ] **Step 1: Write the failing unit tests**

Append inside the `mod tests` of `crates/edi835_core/src/delimiters.rs`:

```rust
    #[test]
    fn is_special_covers_every_configured_delimiter() {
        let d = Delimiters::new(b'*', b':', b'~').with_repetition(b'^').with_release(b'?');
        for byte in [b'*', b':', b'~', b'^', b'?'] {
            assert!(d.is_special(byte), "{}", byte as char);
        }
        assert!(!d.is_special(b'A'));
        assert!(!Delimiters::new(b'*', b':', b'~').is_special(b'^'));
    }
```

Append inside the `mod tests` of `crates/edi835_core/src/segment.rs`:

```rust
    fn written(segment: &Segment<'_>, delims: &Delimiters) -> Result<Vec<u8>, WriteError> {
        let mut out = Vec::new();
        segment.write_to(delims, &mut out)?;
        Ok(out)
    }

    #[test]
    fn write_to_rebuilds_id_elements_and_terminator() {
        let delims = Delimiters::new(b'*', b':', b'~');
        let segment = Segment::parse(0, frame(b"SVC*HC:99213*100**12"), &delims);
        assert_eq!(written(&segment, &delims).unwrap(), b"SVC*HC:99213*100**12~");
    }

    #[test]
    fn write_to_escapes_delimiters_inside_values_when_release_is_set() {
        let delims = Delimiters::new(b'*', b':', b'~').with_release(b'?');
        let segment = Segment {
            index: 0,
            raw: b"",
            id: b"N1",
            elements: vec![Element::Simple(Cow::Owned(b"A*B~C?D".to_vec()))],
            terminated: true,
        };
        assert_eq!(written(&segment, &delims).unwrap(), b"N1*A?*B?~C??D~");
    }

    #[test]
    fn write_to_without_release_rejects_a_delimiter_in_a_value() {
        let delims = Delimiters::new(b'*', b':', b'~');
        let segment = Segment {
            index: 0,
            raw: b"",
            id: b"N1",
            elements: vec![Element::Simple(Cow::Borrowed(b"A*B"))],
            terminated: true,
        };
        assert!(matches!(
            written(&segment, &delims),
            Err(WriteError::DelimiterInValue { byte: b'*' })
        ));
    }

    #[test]
    fn write_to_does_not_write_trivia_from_raw() {
        let delims = Delimiters::new(b'*', b':', b'~');
        let segment = Segment::parse(0, Frame { raw: b"\nSE*2*1~", body: b"SE*2*1", terminated: true }, &delims);
        assert_eq!(written(&segment, &delims).unwrap(), b"SE*2*1~");
    }

    #[test]
    fn write_error_displays_a_message() {
        assert_eq!(
            WriteError::DelimiterInValue { byte: b'*' }.to_string(),
            "value contains delimiter byte 0x2A and no release byte is configured"
        );
    }
```

Add `pub use segment::{Segment, WriteError};` in `crates/edi835_core/src/lib.rs` (replacing `pub use segment::Segment;`).

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p edi835_core --lib`
Expected: compile error — `no method named is_special` / `write_to`, `cannot find type WriteError`.

- [ ] **Step 3: Implement `is_special`**

Add inside `impl Delimiters` in `crates/edi835_core/src/delimiters.rs`:

```rust
    /// `true` when `byte` is any configured delimiter and must be escaped on output.
    pub fn is_special(&self, byte: u8) -> bool {
        byte == self.element
            || byte == self.component
            || byte == self.segment
            || self.repetition == Some(byte)
            || self.release == Some(byte)
    }
```

- [ ] **Step 4: Implement `WriteError` and `write_to`**

In `crates/edi835_core/src/segment.rs`, extend the imports:

```rust
use std::fmt;
use std::io;

use crate::delimiters::Delimiters;
use crate::element::{Element, split_raw};
use crate::frame::Frame;
```

and append after the existing `impl<'a> Segment<'a>` block:

```rust
/// Why a segment could not be written.
#[derive(Debug)]
pub enum WriteError {
    /// The sink failed.
    Io(io::Error),
    /// A value contains a delimiter and no release byte is configured to escape it.
    DelimiterInValue {
        /// The offending byte.
        byte: u8,
    },
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteError::Io(e) => write!(f, "write failed: {e}"),
            WriteError::DelimiterInValue { byte } => write!(
                f,
                "value contains delimiter byte 0x{byte:02X} and no release byte is configured"
            ),
        }
    }
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            WriteError::Io(e) => Some(e),
            WriteError::DelimiterInValue { .. } => None,
        }
    }
}

impl From<io::Error> for WriteError {
    fn from(e: io::Error) -> Self {
        WriteError::Io(e)
    }
}

impl Segment<'_> {
    /// Writes `id`, `elements` and the terminator using `delims` (the symmetric
    /// half of N2). Delimiter bytes inside values are escaped with the release
    /// byte; without one they are an error. Trivia in `raw` is not written:
    /// this is the writer's path, not the lossless one.
    pub fn write_to<W: io::Write>(&self, delims: &Delimiters, out: &mut W) -> Result<(), WriteError> {
        out.write_all(self.id)?;
        for element in &self.elements {
            out.write_all(&[delims.element])?;
            match element {
                Element::Simple(value) => write_value(value, delims, out)?,
                Element::Composite(values) => {
                    for (i, value) in values.iter().enumerate() {
                        if i > 0 {
                            out.write_all(&[delims.component])?;
                        }
                        write_value(value, delims, out)?;
                    }
                }
            }
        }
        out.write_all(&[delims.segment])?;
        Ok(())
    }
}

fn write_value<W: io::Write>(value: &[u8], delims: &Delimiters, out: &mut W) -> Result<(), WriteError> {
    if !value.iter().any(|&byte| delims.is_special(byte)) {
        return Ok(out.write_all(value)?);
    }
    let Some(release) = delims.release else {
        let byte = value.iter().copied().find(|&byte| delims.is_special(byte)).unwrap_or_default();
        return Err(WriteError::DelimiterInValue { byte });
    };
    for &byte in value {
        if delims.is_special(byte) {
            out.write_all(&[release, byte])?;
        } else {
            out.write_all(&[byte])?;
        }
    }
    Ok(())
}
```

- [ ] **Step 5: Run the unit tests to verify they pass**

Run: `cargo test -p edi835_core --lib`
Expected: `test result: ok. 50 passed` (44 + 1 delimiters + 5 segment).

- [ ] **Step 6: Write the round-trip property tests**

Create `crates/edi835_core/tests/roundtrip_props.rs`:

```rust
//! Properties of the symmetric writer: write ∘ tokenize is the identity on
//! (id, elements), and on real files the writer reproduces each segment's bytes.

mod common;

use std::borrow::Cow;

use edi835_core::{Delimiters, Element, Segment, Tokenizer, frame::is_trivia};
use proptest::prelude::*;

const ELEMENT: u8 = b'*';
const COMPONENT: u8 = b':';
const SEGMENT: u8 = b'~';
const RELEASE: u8 = b'?';

/// Bytes that are never a delimiter, so a value survives with or without a release byte.
fn plain_value() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        any::<u8>().prop_filter("not a delimiter", |b| !matches!(*b, ELEMENT | COMPONENT | SEGMENT | RELEASE)),
        0..8,
    )
}

/// Any bytes at all, delimiters included: only a release byte can carry these.
fn any_value() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 0..8)
}

fn segment_id() -> impl Strategy<Value = Vec<u8>> {
    "[A-Z][A-Z0-9]{1,2}".prop_map(String::into_bytes)
}

/// One value → simple element; two or more → composite.
fn elements_from(values: Vec<Vec<Vec<u8>>>) -> Vec<Element<'static>> {
    values
        .into_iter()
        .map(|mut vs| {
            if vs.len() == 1 {
                Element::Simple(Cow::Owned(vs.remove(0)))
            } else {
                Element::Composite(vs.into_iter().map(Cow::Owned).collect())
            }
        })
        .collect()
}

fn roundtrip(id: &[u8], elements: Vec<Element<'static>>, delims: Delimiters) -> Result<(), TestCaseError> {
    let segment = Segment { index: 0, raw: b"", id, elements: elements.clone(), terminated: true };
    let mut written = Vec::new();
    segment.write_to(&delims, &mut written).map_err(|e| TestCaseError::fail(e.to_string()))?;
    let back: Vec<Segment<'_>> = Tokenizer::with_delimiters(&written, delims).collect();
    prop_assert_eq!(back.len(), 1, "written bytes: {:?}", written);
    prop_assert_eq!(back[0].id, id);
    prop_assert_eq!(&back[0].elements, &elements);
    Ok(())
}

proptest! {
    #[test]
    fn write_then_tokenize_restores_id_and_elements(
        id in segment_id(),
        values in prop::collection::vec(prop::collection::vec(plain_value(), 1..3), 0..6),
    ) {
        roundtrip(&id, elements_from(values), Delimiters::new(ELEMENT, COMPONENT, SEGMENT))?;
    }

    #[test]
    fn with_a_release_byte_any_value_survives(
        id in segment_id(),
        values in prop::collection::vec(prop::collection::vec(any_value(), 1..3), 0..6),
    ) {
        let delims = Delimiters::new(ELEMENT, COMPONENT, SEGMENT).with_release(RELEASE);
        roundtrip(&id, elements_from(values), delims)?;
    }
}

/// On the real files the writer reproduces every non-empty segment's bytes exactly
/// (its `raw` minus leading trivia): nothing is normalised on the way out.
#[test]
fn writer_reproduces_every_fixture_segment_byte_for_byte() {
    for name in [
        "emedny_sample.txt",
        "united_healthcare_legacy_sample.txt",
        "multi_claim_sample.txt",
        "trizetto_sample.rmt",
    ] {
        let bytes = common::load_fixture(name);
        let tokenizer = Tokenizer::new(&bytes).unwrap();
        let delims = *tokenizer.delimiters();
        for segment in tokenizer.filter(|s| !s.is_empty()) {
            let mut written = Vec::new();
            segment.write_to(&delims, &mut written).unwrap();
            let body_start = segment.raw.iter().position(|&b| !is_trivia(b)).unwrap_or(0);
            assert_eq!(written, &segment.raw[body_start..], "{name} segment {}", segment.index);
        }
    }
}
```

- [ ] **Step 7: Run the property tests**

Run: `cargo test -p edi835_core --test roundtrip_props`
Expected: `test result: ok. 3 passed`. A failure prints the minimal counterexample; the fix goes in `write_to`/`unescape`/`split_raw`, never in the property.

- [ ] **Step 8: Lint, format, full suite, commit**

Run: `cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all && cargo fmt --all -- --check && cargo test --workspace --locked`
Expected: clean; suite green (lib 50 + frame_props 2 + roundtrip_props 3 + tokenize_fixtures 5 + fixtures_harness 1).

```bash
git add -A crates/edi835_core
git commit -m "feat(segment): symmetric write_to with release escaping; round-trip properties"
```

---

## Task 6: Benchmark baseline and status docs

**Files:**
- Create: `crates/edi835_core/benches/tokenize.rs`
- Delete: `crates/edi835_core/benches/smoke.rs`
- Modify: `crates/edi835_core/Cargo.toml` (`[[bench]]` name)
- Modify: `README.md` (Status)

**Interfaces:**
- Consumes: `Tokenizer::new` (Task 4).
- Produces: nothing code-level; a recorded baseline.

- [ ] **Step 1: Replace the smoke bench with a tokenizer throughput bench**

Run: `git rm crates/edi835_core/benches/smoke.rs`

In `crates/edi835_core/Cargo.toml`, change the bench block to:

```toml
[[bench]]
name = "tokenize"
harness = false
```

Create `crates/edi835_core/benches/tokenize.rs`:

```rust
//! Stage 1 baseline: tokenizer throughput over the three largest fixtures.
//! No threshold yet (N4: measure first, then decide what "fast" means).

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use edi835_core::Tokenizer;
use std::hint::black_box;

const FIXTURES: &[&str] = &[
    "emedny_sample.txt",
    "united_healthcare_legacy_sample.txt",
    "multi_claim_sample.txt",
];

fn load(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

fn tokenize_fixtures(c: &mut Criterion) {
    let mut group = c.benchmark_group("tokenize");
    for name in FIXTURES {
        let bytes = load(name);
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), &bytes, |b, bytes| {
            b.iter(|| Tokenizer::new(black_box(bytes)).expect("fixture has an ISA").count());
        });
    }
    group.finish();
}

criterion_group!(benches, tokenize_fixtures);
criterion_main!(benches);
```

- [ ] **Step 2: Compile and run the bench, record the baseline**

Run: `cargo bench --workspace --no-run --locked && cargo bench --workspace 2>&1 | grep -E 'tokenize/|time:|thrpt:'`
Expected: three `tokenize/<fixture>` entries, each with `time:` and `thrpt:` lines (MiB/s), exit 0. Copy the three `thrpt:` lines into the commit message body of Step 4 so the baseline is in git history.

- [ ] **Step 3: Update the README status**

In `README.md`, replace the `## Status` section with:

```markdown
## Status

**Stage 1 — framing + tokenizer.** Bytes → lazy, lossless `Segment` stream, delimiters
read from the ISA, symmetric writer. No 835 knowledge yet (that is Stage 3 data).
```

- [ ] **Step 4: Final sweep and commit**

Run: `cargo build --workspace --all-targets --locked && cargo test --workspace --locked && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all -- --check && cargo bench --workspace --no-run --locked`
Expected: every command exits 0.

```bash
git add -A
git commit -m "bench: tokenizer throughput baseline over fixtures; README status to Stage 1

Baseline (criterion, <machine>):
  <paste the three thrpt lines here>"
```

---

## Stage 1 exit gate (definition of done)

All of these must hold before Stage 2:

- [ ] `cargo test --workspace --locked` passes: 50 unit tests, `frame_props` 2, `roundtrip_props` 3, `tokenize_fixtures` 5, `fixtures_harness` 1.
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo fmt --all -- --check` clean.
- [ ] `cargo bench` runs the `tokenize` group and the baseline is recorded in a commit message.
- [ ] `[dependencies]` of `edi835_core` is still empty.
- [ ] No `unwrap`/`expect`/`panic!` in `crates/edi835_core/src/` (check: `grep -rnE 'unwrap\(|expect\(|panic!' crates/edi835_core/src/ | grep -v '#\[cfg(test)\]' ` returns only lines inside `mod tests`).
- [ ] Fixtures unchanged: `git diff --stat HEAD~6 -- crates/edi835_core/tests/fixtures/*.txt crates/edi835_core/tests/fixtures/*.rmt` is empty.
- [ ] `.doc/architectural-commitment.md` §7 Stage 1 is marked APROBADO by the project owner (not by the executor).
