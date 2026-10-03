# Stage 2 — Documento lossless · Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `Document` that holds a whole EDI file losslessly, borrows the caller's bytes when it can and owns them when asked, and yields exactly the same `Segment`s as the `Tokenizer`.

**Architecture:** `Document<'a>` stores the bytes as `Cow<'a, [u8]>` plus one `Span` (byte ranges) per segment. Building it only runs the framing pass; `Segment`s are parsed on demand from a span, borrowing from the document. `into_owned()` turns any document into a `Document<'static>` by copying the bytes once; spans are reused.

**Tech Stack:** Rust edition 2024 (stable), `proptest` 1 and `criterion` 0.5 (already dev-dependencies). No new dependencies.

**Spec:** `.doc/architectural-commitment.md` — §7 "Stage 2 · Documento lossless" and decision T5 in §6.1, argued from N1, N4, N7, P9.

> **All commands run from the project root** `/home/javillegasna/Desktop/org/personal/oxedi835/`.

## Global Constraints

- Edition 2024, stable toolchain. `[dependencies]` of `edi835_core` stays empty.
- Every commit passes `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo fmt --all -- --check`.
- No `unwrap`, `expect`, `panic!`, or fallible indexing on arbitrary input in `src/`. Tests may unwrap.
- Comments describe implementation only: no stage numbers, principle codes or history.
- `Document::segment(i)` must equal the tokenizer's i-th `Segment` field by field (`index`, `raw`, `id`, `elements`, `terminated`).
- Building a `Document` must not parse elements (framing only).
- Fixtures are never modified.

## Review Focus

1. **A span computed from a frame whose trivia and terminator lengths are both zero** (unterminated, trivia-only tail) — `body` must be an empty range at the right offset, not a reversed or out-of-bounds range. Test: `spans_partition_the_bytes` with `b"ST~\n"` (Task 1).
2. **`Document::parse` over a `Vec<u8>`** — the resulting type must be `Document<'static>` and must not borrow from the vector's old location. Test: `parse_accepts_owned_bytes` (Task 2).
3. **Dropping the original buffer after `into_owned()`** — must compile and keep working; if it borrowed, the compiler would refuse. Test: `into_owned_detaches_from_the_buffer` (Task 2).
4. **Empty input** — a document with zero spans, `as_bytes()` empty, iterators yield nothing, no panic. Test: `empty_input_is_an_empty_document` (Task 1).
5. **A document built with a release byte** — on-demand `Segment` parsing must apply the same `release` the tokenizer would. Property: `document_segments_equal_tokenizer_segments` with `use_release` (Task 2).

---

## File Structure

```
crates/edi835_core/
├── src/
│   ├── lib.rs                  # + pub mod document; re-export Document, Segments, Span
│   └── document.rs             # Span, Document, Segments (iterator), index()
├── benches/
│   └── tokenize.rs             # + "index" group: Document::parse over the same fixtures
└── tests/
    ├── document_props.rs       # proptest: document == tokenizer; spans partition the bytes
    └── document_fixtures.rs    # five fixtures through Document, borrowed and owned
```

`document.rs` depends on `delimiters`, `frame` and `segment`; nothing depends on it except `lib.rs`.

---

## Task 1: `Span` and `Document` construction with on-demand `segment(i)`

**Files:**
- Create: `crates/edi835_core/src/document.rs`
- Modify: `crates/edi835_core/src/lib.rs`

**Interfaces:**
- Consumes: `Delimiters`, `IsaError` (`delimiters`); `Frame`, `next_frame`, `is_trivia` (`frame`); `Segment::parse` (`segment`).
- Produces:
  - `pub struct Span { pub raw: Range<usize>, pub body: Range<usize>, pub terminated: bool }` (`Clone`, `Eq`, `Debug`)
  - `pub struct Document<'a>` (`Clone`, `Eq`, `Debug`) with private fields `bytes: Cow<'a, [u8]>`, `delims: Delimiters`, `spans: Vec<Span>`
  - `Document::parse(bytes: impl Into<Cow<'a, [u8]>>) -> Result<Document<'a>, IsaError>`
  - `Document::with_delimiters(bytes: impl Into<Cow<'a, [u8]>>, delims: Delimiters) -> Document<'a>`
  - `Document::as_bytes(&self) -> &[u8]`, `delimiters(&self) -> &Delimiters`, `spans(&self) -> &[Span]`, `len(&self) -> usize`, `is_empty(&self) -> bool`
  - `Document::segment(&self, index: usize) -> Option<Segment<'_>>`

- [ ] **Step 1: Write the failing tests**

Create `crates/edi835_core/src/document.rs` with only:

```rust
//! A whole EDI file held losslessly: the bytes plus one span per segment.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Delimiters, Tokenizer};

    const ISA: &[u8] =
        b"ISA*00*          *00*          *ZZ*EMEDNYBAT      *ZZ*ETIN           *100101*1000*^*00501*006000600*0*T*:~";

    fn plain() -> Delimiters {
        Delimiters::new(b'*', b':', b'~')
    }

    #[test]
    fn with_delimiters_indexes_every_frame() {
        let doc = Document::with_delimiters(&b"ST*835~\nSE*2~\n"[..], plain());
        assert_eq!(doc.len(), 3);
        assert_eq!(doc.segment(0).unwrap().id, b"ST");
        assert_eq!(doc.segment(1).unwrap().id, b"SE");
        assert!(doc.segment(2).unwrap().is_empty());
        assert_eq!(doc.segment(3), None);
    }

    #[test]
    fn spans_partition_the_bytes() {
        let input = &b"\nST*835~\n\nSE~X"[..];
        let doc = Document::with_delimiters(input, plain());
        let spans = doc.spans();
        assert_eq!(spans[0], Span { raw: 0..8, body: 1..7, terminated: true });
        assert_eq!(spans[1], Span { raw: 8..13, body: 10..12, terminated: true });
        assert_eq!(spans[2], Span { raw: 13..14, body: 13..14, terminated: false });
        let mut next = 0;
        for span in spans {
            assert_eq!(span.raw.start, next, "spans must be contiguous");
            assert!(span.raw.start <= span.body.start && span.body.end <= span.raw.end);
            next = span.raw.end;
        }
        assert_eq!(next, input.len(), "spans must cover the whole input");

        let tail_only = Document::with_delimiters(&b"ST~\n"[..], plain());
        assert_eq!(tail_only.spans()[1], Span { raw: 3..4, body: 4..4, terminated: false });
    }

    #[test]
    fn segment_equals_tokenizer_output() {
        let input = &b"ISA*00~\r\nSVC*HC:99213*100**12~CAS*CO*45~~"[..];
        let doc = Document::with_delimiters(input, plain());
        let expected: Vec<_> = Tokenizer::with_delimiters(input, plain()).collect();
        assert_eq!(doc.len(), expected.len());
        for (i, segment) in expected.iter().enumerate() {
            assert_eq!(doc.segment(i).as_ref(), Some(segment), "segment {i}");
        }
    }

    #[test]
    fn parse_reads_delimiters_from_the_isa_and_tolerates_leading_trivia() {
        let mut input = b"\r\n".to_vec();
        input.extend_from_slice(ISA);
        input.extend_from_slice(b"GS*HP:X~");
        let doc = Document::parse(&input[..]).unwrap();
        assert_eq!(doc.delimiters().component, b':');
        assert_eq!(doc.segment(0).unwrap().id, b"ISA");
        assert!(doc.segment(0).unwrap().raw.starts_with(b"\r\n"));
        assert_eq!(doc.segment(1).unwrap().element(1).and_then(|e| e.simple()), None);
    }

    #[test]
    fn parse_fails_without_an_isa() {
        assert_eq!(Document::parse(&b"ST*835~"[..]).err(), Some(IsaError::NotIsa));
        assert_eq!(Document::parse(&b""[..]).err(), Some(IsaError::NotIsa));
    }

    #[test]
    fn empty_input_is_an_empty_document() {
        let doc = Document::with_delimiters(&b""[..], plain());
        assert!(doc.is_empty());
        assert_eq!(doc.len(), 0);
        assert_eq!(doc.as_bytes(), b"");
        assert_eq!(doc.segment(0), None);
    }

    #[test]
    fn as_bytes_is_the_input_unchanged() {
        let input = &b"ST*835~\n"[..];
        assert_eq!(Document::with_delimiters(input, plain()).as_bytes(), input);
    }
}
```

In `crates/edi835_core/src/lib.rs`, add `pub mod document;` to the module list (alphabetical, after `delimiters`) and `pub use document::{Document, Span};` to the re-exports.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p edi835_core --lib document`
Expected: compile error — `cannot find type Document` / `Span`.

- [ ] **Step 3: Implement `Span`, `Document` and the index pass**

Insert above the test module in `crates/edi835_core/src/document.rs`:

```rust
//! A whole EDI file held losslessly: the bytes plus one span per segment.
//!
//! Building a document only runs the framing pass and records where each
//! segment lives. Segments are parsed on demand and borrow from the document,
//! so the document can hold either a borrowed slice or an owned buffer without
//! two representations.

use std::borrow::Cow;
use std::ops::Range;

use crate::delimiters::{Delimiters, IsaError};
use crate::frame::{Frame, is_trivia, next_frame};
use crate::segment::Segment;

/// Where one segment lives inside [`Document::as_bytes`].
///
/// `raw` ranges of consecutive spans are contiguous and together cover the
/// whole buffer; `body` lies inside `raw`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// Leading trivia, body and terminator.
    pub raw: Range<usize>,
    /// The segment text without trivia or terminator.
    pub body: Range<usize>,
    /// `false` only for a final segment with no terminator.
    pub terminated: bool,
}

/// A whole file: its bytes and the spans of its segments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document<'a> {
    bytes: Cow<'a, [u8]>,
    delims: Delimiters,
    spans: Vec<Span>,
}

impl<'a> Document<'a> {
    /// Indexes `bytes`, reading the delimiters from the ISA segment (which may
    /// be preceded by trivia). Accepts a borrowed slice or an owned `Vec<u8>`.
    pub fn parse(bytes: impl Into<Cow<'a, [u8]>>) -> Result<Self, IsaError> {
        let bytes = bytes.into();
        let start = bytes.iter().position(|&byte| !is_trivia(byte)).unwrap_or(bytes.len());
        let delims = Delimiters::from_isa(&bytes[start..])?;
        Ok(Self::with_delimiters(bytes, delims))
    }

    /// Indexes `bytes` with caller-supplied delimiters.
    pub fn with_delimiters(bytes: impl Into<Cow<'a, [u8]>>, delims: Delimiters) -> Self {
        let bytes = bytes.into();
        let spans = index(&bytes, &delims);
        Self { bytes, delims, spans }
    }

    /// The file, byte for byte.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The delimiters in use.
    pub fn delimiters(&self) -> &Delimiters {
        &self.delims
    }

    /// One span per segment, in order.
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    /// Number of segments, including empty and trivia-only ones.
    pub fn len(&self) -> usize {
        self.spans.len()
    }

    /// `true` when the input had no bytes at all.
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// The segment at `index` (0-based, same as [`Segment::index`]), parsed on demand.
    pub fn segment(&self, index: usize) -> Option<Segment<'_>> {
        let span = self.spans.get(index)?;
        Some(self.segment_from(index, span))
    }

    fn segment_from(&self, index: usize, span: &Span) -> Segment<'_> {
        let frame = Frame {
            raw: &self.bytes[span.raw.clone()],
            body: &self.bytes[span.body.clone()],
            terminated: span.terminated,
        };
        Segment::parse(index, frame, &self.delims)
    }
}

/// Runs the framing pass and records each frame as byte ranges.
fn index(bytes: &[u8], delims: &Delimiters) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut rest = bytes;
    let mut offset = 0;
    while let Some((frame, next)) = next_frame(rest, delims) {
        // raw = trivia + body + terminator, so the body offset is what is left
        // after removing the body and the (0 or 1 byte) terminator from raw.
        let trivia = frame.raw.len() - frame.body.len() - usize::from(frame.terminated);
        let raw = offset..offset + frame.raw.len();
        let body = raw.start + trivia..raw.start + trivia + frame.body.len();
        spans.push(Span { raw: raw.clone(), body, terminated: frame.terminated });
        offset = raw.end;
        rest = next;
    }
    spans
}

```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p edi835_core --lib document`
Expected: `test result: ok. 7 passed`.

- [ ] **Step 5: Lint, format, commit**

Run: `cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all && cargo fmt --all -- --check && cargo test --workspace --locked`
Expected: clean; suite green (lib 57 + the Stage 1 integration tests).

```bash
git add crates/edi835_core/src/document.rs crates/edi835_core/src/lib.rs
git commit -m "feat(document): lossless Document as Cow bytes plus spans, segments on demand"
```

---

## Task 2: Iteration, ownership, and the seam with the tokenizer

**Files:**
- Modify: `crates/edi835_core/src/document.rs`
- Modify: `crates/edi835_core/src/lib.rs` (re-export `Segments`)
- Create: `crates/edi835_core/tests/document_props.rs`
- Create: `crates/edi835_core/tests/document_fixtures.rs`

**Interfaces:**
- Consumes: everything from Task 1; `common::load_fixture`; `Tokenizer`.
- Produces:
  - `pub struct Segments<'d, 'a>` implementing `Iterator<Item = Segment<'d>>` (and `ExactSizeIterator`)
  - `Document::segments(&self) -> Segments<'_, 'a>`
  - `impl<'d, 'a> IntoIterator for &'d Document<'a>` with `Item = Segment<'d>`, `IntoIter = Segments<'d, 'a>`
  - `Document::into_owned(self) -> Document<'static>`

- [ ] **Step 1: Write the failing unit tests**

Append inside `mod tests` of `crates/edi835_core/src/document.rs`:

```rust
    #[test]
    fn segments_iterator_yields_every_segment_with_consecutive_indices() {
        let doc = Document::with_delimiters(&b"ST*835~SE*2~\n"[..], plain());
        let segments: Vec<_> = doc.segments().collect();
        assert_eq!(segments.len(), 3);
        assert_eq!(doc.segments().len(), 3, "ExactSizeIterator");
        for (i, segment) in segments.iter().enumerate() {
            assert_eq!(segment.index, i);
            assert_eq!(doc.segment(i).as_ref(), Some(segment));
        }
    }

    #[test]
    fn a_document_reference_can_be_iterated_with_for() {
        let doc = Document::with_delimiters(&b"ST*835~SE*2~"[..], plain());
        let mut ids = Vec::new();
        for segment in &doc {
            ids.push(segment.id);
        }
        assert_eq!(ids, vec![&b"ST"[..], b"SE"]);
    }

    #[test]
    fn parse_accepts_owned_bytes() {
        fn assert_static(_: &Document<'static>) {}
        let mut input = ISA.to_vec();
        input.extend_from_slice(b"GS*HP~");
        let doc = Document::parse(input).unwrap();
        assert_static(&doc);
        assert_eq!(doc.len(), 2);
        assert_eq!(doc.segment(1).unwrap().id, b"GS");
    }

    #[test]
    fn into_owned_detaches_from_the_buffer() {
        fn assert_static(_: &Document<'static>) {}
        let buffer = b"ST*835~SE*2~".to_vec();
        let borrowed = Document::with_delimiters(&buffer[..], plain());
        let before: Vec<_> = borrowed.segments().collect();
        let owned = borrowed.into_owned();
        drop(buffer);
        assert_static(&owned);
        let after: Vec<_> = owned.segments().collect();
        assert_eq!(after, before);
    }

    #[test]
    fn into_owned_of_an_owned_document_does_not_change_it() {
        let doc = Document::with_delimiters(b"ST*835~".to_vec(), plain());
        let owned = doc.clone().into_owned();
        assert_eq!(owned, doc);
    }
```

Change the `lib.rs` re-export to `pub use document::{Document, Segments, Span};`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p edi835_core --lib document`
Expected: compile error — `no method named segments` / `into_owned`, `Document` is not an iterator (`for segment in &doc`).

- [ ] **Step 3: Implement `Segments`, `segments`, `IntoIterator` and `into_owned`**

Add inside `impl<'a> Document<'a>` (after `segment`):

```rust
    /// Iterates every segment in order, parsing each on demand.
    pub fn segments(&self) -> Segments<'_, 'a> {
        Segments { doc: self, next: 0 }
    }

    /// Makes the document own its bytes, copying them only if they were borrowed.
    /// Spans are reused as they are.
    pub fn into_owned(self) -> Document<'static> {
        Document {
            bytes: Cow::Owned(self.bytes.into_owned()),
            delims: self.delims,
            spans: self.spans,
        }
    }
```

Add after the `impl<'a> Document<'a>` block (before `fn index`):

```rust
impl<'d, 'a> IntoIterator for &'d Document<'a> {
    type Item = Segment<'d>;
    type IntoIter = Segments<'d, 'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.segments()
    }
}

/// Iterator over a document's segments. `'d` is the borrow of the document,
/// `'a` the document's own buffer lifetime; items borrow for `'d`.
#[derive(Debug, Clone)]
pub struct Segments<'d, 'a> {
    doc: &'d Document<'a>,
    next: usize,
}

impl<'d> Iterator for Segments<'d, '_> {
    type Item = Segment<'d>;

    fn next(&mut self) -> Option<Self::Item> {
        let segment = self.doc.segment(self.next)?;
        self.next += 1;
        Some(segment)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.doc.len().saturating_sub(self.next);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for Segments<'_, '_> {}
```

- [ ] **Step 4: Run the unit tests to verify they pass**

Run: `cargo test -p edi835_core --lib document`
Expected: `test result: ok. 12 passed`.

- [ ] **Step 5: Write the property and fixture tests**

Create `crates/edi835_core/tests/document_props.rs`:

```rust
//! Properties of the document: it yields exactly what the tokenizer yields, and
//! its spans partition the input.

use edi835_core::{Delimiters, Document, Tokenizer};
use proptest::prelude::*;

fn delimiters(use_release: bool) -> Delimiters {
    let delims = Delimiters::new(b'*', b':', b'~');
    if use_release {
        delims.with_release(b'?')
    } else {
        delims
    }
}

proptest! {
    #[test]
    fn document_segments_equal_tokenizer_segments(
        input in prop::collection::vec(any::<u8>(), 0..256),
        use_release in any::<bool>(),
    ) {
        let delims = delimiters(use_release);
        let doc = Document::with_delimiters(&input[..], delims);
        let from_doc: Vec<_> = doc.segments().collect();
        let from_tokenizer: Vec<_> = Tokenizer::with_delimiters(&input, delims).collect();
        prop_assert_eq!(from_doc, from_tokenizer);
    }

    #[test]
    fn spans_partition_the_input(
        input in prop::collection::vec(any::<u8>(), 0..256),
        use_release in any::<bool>(),
    ) {
        let doc = Document::with_delimiters(&input[..], delimiters(use_release));
        let mut next = 0;
        for span in doc.spans() {
            prop_assert_eq!(span.raw.start, next);
            prop_assert!(span.raw.start <= span.body.start);
            prop_assert!(span.body.end <= span.raw.end);
            next = span.raw.end;
        }
        prop_assert_eq!(next, input.len());
    }

    #[test]
    fn owned_document_equals_borrowed_document(
        input in prop::collection::vec(any::<u8>(), 0..256),
    ) {
        let delims = delimiters(false);
        let borrowed = Document::with_delimiters(&input[..], delims);
        let from_borrowed: Vec<_> = borrowed.segments().collect();
        let owned = borrowed.into_owned();
        let from_owned: Vec<_> = owned.segments().collect();
        prop_assert_eq!(from_owned, from_borrowed);
        prop_assert_eq!(owned.as_bytes(), &input[..]);
    }
}
```

Create `crates/edi835_core/tests/document_fixtures.rs`:

```rust
//! The five real fixtures through `Document`: lossless, identical to the
//! tokenizer, borrowed and owned.

mod common;

use edi835_core::{Delimiters, Document, IsaError, Tokenizer};

const ENVELOPED: &[&str] = &[
    "emedny_sample.txt",
    "united_healthcare_legacy_sample.txt",
    "multi_claim_sample.txt",
    "trizetto_sample.rmt",
];

fn assert_matches_tokenizer(doc: &Document<'_>, bytes: &[u8], delims: Delimiters, name: &str) {
    assert_eq!(doc.as_bytes(), bytes, "{name}: bytes must be the file");
    let expected: Vec<_> = Tokenizer::with_delimiters(bytes, delims).collect();
    assert_eq!(doc.len(), expected.len(), "{name}: segment count");
    for (i, segment) in expected.iter().enumerate() {
        assert_eq!(doc.segment(i).as_ref(), Some(segment), "{name} segment {i}");
    }
    let from_iter: Vec<_> = doc.segments().collect();
    assert_eq!(from_iter, expected, "{name}: segments() must match the tokenizer");
    let rebuilt: Vec<u8> = doc.spans().iter().flat_map(|s| bytes[s.raw.clone()].iter().copied()).collect();
    assert_eq!(rebuilt, bytes, "{name}: spans must rebuild the file");
}

#[test]
fn enveloped_fixtures_are_held_losslessly() {
    for name in ENVELOPED {
        let bytes = common::load_fixture(name);
        let doc = Document::parse(&bytes[..]).unwrap_or_else(|e| panic!("{name}: {e}"));
        let delims = *doc.delimiters();
        assert_matches_tokenizer(&doc, &bytes, delims, name);
    }
}

#[test]
fn owned_fixtures_yield_the_same_segments() {
    for name in ENVELOPED {
        let bytes = common::load_fixture(name);
        let delims = *Document::parse(&bytes[..]).unwrap().delimiters();
        let owned = Document::parse(bytes.clone()).unwrap();
        assert_matches_tokenizer(&owned, &bytes, delims, name);
        let promoted = Document::parse(&bytes[..]).unwrap().into_owned();
        assert_eq!(promoted, owned, "{name}: into_owned must equal parsing an owned Vec");
    }
}

#[test]
fn fragment_without_isa_needs_caller_delimiters() {
    let bytes = common::load_fixture("blue_cross_nc_sample.txt");
    assert_eq!(Document::parse(&bytes[..]).err(), Some(IsaError::NotIsa));
    let delims = Delimiters::new(b'*', b':', b'~');
    let doc = Document::with_delimiters(&bytes[..], delims);
    assert_eq!(doc.segment(0).unwrap().id, b"ST");
    assert_matches_tokenizer(&doc, &bytes, delims, "blue_cross_nc_sample.txt");
}
```

- [ ] **Step 6: Run the property and fixture tests**

Run: `cargo test -p edi835_core --test document_props --test document_fixtures`
Expected: `test result: ok. 3 passed` and `test result: ok. 3 passed`. A property failure prints the minimal counterexample; fix `index()` or `segment_from`, never the property.

- [ ] **Step 7: Lint, format, full suite, commit**

Run: `cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all && cargo fmt --all -- --check && cargo test --workspace --locked`
Expected: clean; suite green (lib 62, document_props 3, document_fixtures 3, plus Stage 1's frame_props 2, roundtrip_props 3, tokenize_fixtures 5, fixtures_harness 1).

```bash
git add -A crates/edi835_core
git commit -m "feat(document): segments iterator, IntoIterator for &Document, into_owned; seam with tokenizer proven"
```

---

## Task 3: Benchmark the index pass and update the status

**Files:**
- Modify: `crates/edi835_core/benches/tokenize.rs`
- Modify: `README.md`

**Interfaces:**
- Consumes: `Document::parse`.
- Produces: a recorded baseline for indexing next to tokenizing.

- [ ] **Step 1: Add the index group to the bench**

In `crates/edi835_core/benches/tokenize.rs`, change the import to `use edi835_core::{Document, Tokenizer};`, add this function after `tokenize_fixtures`, and register it:

```rust
fn index_fixtures(c: &mut Criterion) {
    let mut group = c.benchmark_group("index");
    for name in FIXTURES {
        let bytes = load(name);
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), &bytes, |b, bytes| {
            b.iter(|| Document::parse(black_box(&bytes[..])).expect("fixture has an ISA").len());
        });
    }
    group.finish();
}

criterion_group!(benches, tokenize_fixtures, index_fixtures);
```

(replace the existing `criterion_group!(benches, tokenize_fixtures);` line.)

- [ ] **Step 2: Run the bench and record both groups**

Run: `cargo bench --workspace --no-run --locked && cargo bench --workspace 2>&1 | grep -E '^(tokenize|index)/|thrpt:'`
Expected: six entries, three per group, each with a `thrpt:` line; `index/*` throughput higher than `tokenize/*` for the same fixture (indexing skips element parsing). Copy the six `thrpt:` lines into the commit message of Step 4.

- [ ] **Step 3: Update the README status**

Replace the `## Status` section of `README.md` with:

```markdown
## Status

**Stage 2 — lossless document.** Bytes → lazy `Segment` stream (Stage 1) or a `Document`
that holds the whole file, borrowed or owned, and yields the same segments on demand.
No 835 knowledge yet (that is Stage 3 data).
```

- [ ] **Step 4: Final sweep and commit**

Run: `cargo build --workspace --all-targets --locked && cargo test --workspace --locked && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo fmt --all -- --check && cargo bench --workspace --no-run --locked`
Expected: every command exits 0.

```bash
git add -A
git commit -m "bench: index pass baseline next to tokenize; README status to Stage 2

Baseline (criterion, <machine>):
  <paste the six thrpt lines here>"
```

---

## Stage 2 exit gate (definition of done)

- [ ] `cargo test --workspace --locked` passes: 62 unit tests, `document_props` 3, `document_fixtures` 3, and the Stage 1 suites.
- [ ] clippy with `-D warnings` and `fmt --check` clean.
- [ ] `cargo bench` runs the `index` group and the baseline is in a commit message.
- [ ] `[dependencies]` of `edi835_core` is still empty.
- [ ] No `unwrap`/`expect`/`panic!` in `src/` outside `mod tests`.
- [ ] Fixtures unchanged.
- [ ] `.doc/architectural-commitment.md` §7 Stage 2 marked APROBADO by the project owner.
