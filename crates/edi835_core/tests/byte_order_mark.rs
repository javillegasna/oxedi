//! A UTF-8 byte order mark before the first segment: the file round-trips
//! byte for byte, the mark is leading trivia of segment 0, and everything the
//! processor derives is what the same file yields without it, plus one
//! informational diagnostic.

mod common;

use edi835_core::{
    Delimiters, Diagnostic, Document, Event, Processor, Rule, Segment, SnipLevel, Spec, Tables,
};
use proptest::prelude::*;

const BOM: &[u8] = b"\xEF\xBB\xBF";

/// Every event, every diagnostic and the tables of one pass over `document`.
fn run(spec: &Spec, document: &Document<'_>) -> (Vec<Event>, Vec<Diagnostic>, Tables) {
    let mut processor = Processor::new(spec, document.delimiters());
    let mut events = Vec::new();
    let mut diagnostics = Vec::new();
    for segment in document.segments() {
        let output = processor.feed(&segment);
        events.extend_from_slice(output.events());
        diagnostics.extend_from_slice(output.diagnostics());
    }
    let output = processor.finish();
    events.extend_from_slice(output.events());
    diagnostics.extend_from_slice(output.diagnostics());
    (events, diagnostics, processor.take_tables())
}

fn prefixed(bytes: &[u8]) -> Vec<u8> {
    [BOM, bytes].concat()
}

/// The marked document holds exactly the marked bytes; its segments are the
/// plain document's, the first one's `raw` led by the mark.
fn assert_same_segments(marked: &Document<'_>, plain: &Document<'_>) -> Result<(), TestCaseError> {
    prop_assert_eq!(marked.as_bytes(), &prefixed(plain.as_bytes())[..]);
    let rebuilt: Vec<u8> = marked
        .segments()
        .flat_map(|segment| segment.raw.to_vec())
        .collect();
    prop_assert_eq!(&rebuilt[..], marked.as_bytes());
    prop_assert_eq!(marked.len(), plain.len().max(1));
    for (index, segment) in marked.segments().enumerate() {
        let expected = plain.segment(index).unwrap_or(Segment {
            index: 0,
            raw: b"",
            id: b"",
            elements: Vec::new(),
            terminated: false,
        });
        let raw = if index == 0 {
            prefixed(expected.raw)
        } else {
            expected.raw.to_vec()
        };
        prop_assert_eq!(segment.raw, &raw[..], "segment {}", index);
        prop_assert_eq!(
            Segment {
                raw: b"",
                ..segment
            },
            Segment {
                raw: b"",
                ..expected
            },
            "segment {}",
            index
        );
    }
    Ok(())
}

/// Same events and tables; the same diagnostics after the mark's own.
fn assert_same_processing(
    spec: &Spec,
    marked: &Document<'_>,
    plain: &Document<'_>,
) -> Result<(), TestCaseError> {
    let (marked_events, mut marked_diagnostics, marked_tables) = run(spec, marked);
    let (plain_events, plain_diagnostics, plain_tables) = run(spec, plain);
    if plain.is_empty() {
        prop_assert_eq!(marked_events.len(), 1, "{:?}", marked_events);
    } else {
        prop_assert_eq!(marked_events, plain_events);
    }
    prop_assert_eq!(marked_tables, plain_tables);
    prop_assert!(!marked_diagnostics.is_empty());
    let mark = marked_diagnostics.remove(0);
    prop_assert_eq!(
        mark,
        Diagnostic {
            rule: Rule::ByteOrderMark,
            level: SnipLevel::L1,
            segment: Some(0),
            element: None,
            component: None,
            path: Vec::new(),
            datum: BOM.to_vec(),
        }
    );
    prop_assert_eq!(marked_diagnostics, plain_diagnostics);
    Ok(())
}

#[test]
fn every_file_with_a_byte_order_mark_round_trips_and_processes_as_without_it() {
    let spec = Spec::builtin_835();
    for (name, bytes, delims) in common::all_files() {
        let marked_bytes = prefixed(&bytes);
        let (marked, plain) = match Document::parse(&bytes[..]) {
            Ok(plain) => (Document::parse(&marked_bytes[..]).expect(&name), plain),
            Err(_) => (
                Document::with_delimiters(&marked_bytes[..], delims),
                Document::with_delimiters(&bytes[..], delims),
            ),
        };
        assert_eq!(marked.delimiters(), plain.delimiters(), "{name}");
        assert_same_segments(&marked, &plain).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_same_processing(&spec, &marked, &plain).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

const ISA: &str = "ISA*00*          *00*          *ZZ*SENDER         *ZZ*RECEIVER       *240101*1200*^*00501*000000001*0*P*>~";

/// Segments an 835 can carry, some well formed and some not, so the
/// generated files open, close, skip and leave loops open in many ways.
const PIECES: &[&str] = &[
    "GS*HP*S*R*20240101*1200*7*X*005010X221A1~",
    "ST*835*0001~",
    "BPR*I*100*C*CHK~",
    "TRN*1*12345*1~",
    "N1*PR*PAYER~",
    "N1*PE*PAYEE*XX*123~",
    "N3*1 MAIN ST~",
    "LX*1~",
    "CLP*A1*1*100*80**MC*X~",
    "CLP*A2*X*abc~",
    "NM1*QC*1*DOE*JOHN****MI*0123~",
    "CAS*CO*45*20~",
    "SVC*HC:99213*100*80**1~",
    "DTM*472*20240101~",
    "PLB*123*20241231*WO:1*5~",
    "ZZZ*1~",
    "~",
    "\r\n",
    "SE*9*0001~",
    "GE*1*7~",
    "IEA*1*000000001~",
];

fn interchange() -> impl Strategy<Value = Vec<u8>> {
    (
        prop::sample::select(vec!["", "\n", " \r\n", "\t"]),
        prop::collection::vec(prop::sample::select(PIECES), 0..24),
        prop::collection::vec(any::<u8>(), 0..8),
    )
        .prop_map(|(lead, pieces, tail)| {
            let mut bytes = format!("{lead}{ISA}").into_bytes();
            for piece in pieces {
                bytes.extend_from_slice(piece.as_bytes());
            }
            bytes.extend_from_slice(&tail);
            bytes
        })
}

proptest! {
    #[test]
    fn an_interchange_with_a_byte_order_mark_processes_as_without_it(bytes in interchange()) {
        let spec = Spec::builtin_835();
        let plain = Document::parse(&bytes[..]).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let marked_bytes = prefixed(&bytes);
        let marked =
            Document::parse(&marked_bytes[..]).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(marked.delimiters(), plain.delimiters());
        assert_same_segments(&marked, &plain)?;
        assert_same_processing(&spec, &marked, &plain)?;
    }

    #[test]
    fn any_input_with_a_byte_order_mark_round_trips_and_processes_as_without_it(
        bytes in prop::collection::vec(any::<u8>(), 0..256)
            .prop_filter("does not start with a second mark", |b| !b.starts_with(BOM)),
    ) {
        let spec = Spec::builtin_835();
        let delims = Delimiters::new(b'*', b':', b'~');
        let marked_bytes = prefixed(&bytes);
        let marked = Document::with_delimiters(&marked_bytes[..], delims);
        let plain = Document::with_delimiters(&bytes[..], delims);
        assert_same_segments(&marked, &plain)?;
        assert_same_processing(&spec, &marked, &plain)?;
    }
}
