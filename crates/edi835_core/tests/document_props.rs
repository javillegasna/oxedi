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
        // The expectation borrows `input`, not the document, so the document
        // can be moved into `into_owned` while it is alive.
        let delims = delimiters(false);
        let expected: Vec<_> = Tokenizer::with_delimiters(&input, delims).collect();
        let owned = Document::with_delimiters(&input[..], delims).into_owned();
        let from_owned: Vec<_> = owned.segments().collect();
        prop_assert_eq!(from_owned, expected);
        prop_assert_eq!(owned.as_bytes(), &input[..]);
    }
}
