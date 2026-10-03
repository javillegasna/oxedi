#![allow(dead_code)] // each test binary compiles this module; not all use every helper
//! Shared helpers for integration tests. Lives in `tests/common/mod.rs` so Cargo
//! treats it as a module (not its own test binary) when included via `mod common;`.

use std::path::PathBuf;

/// Absolute path to this crate's `tests/fixtures` directory (synthetic files).
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Absolute path to this crate's `tests/samples` directory (anonymized real files).
pub fn samples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/samples")
}

/// Read a fixture file's raw bytes by file name. Panics with a clear message if missing.
///
/// Returns bytes, not a `String`: the core consumes `&[u8]` and real payer files
/// are not guaranteed to be UTF-8. The fixture pipeline must never decode on the way in.
pub fn load_fixture(name: &str) -> Vec<u8> {
    let path = fixtures_dir().join(name);
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()))
}

/// Read an anonymized real sample's raw bytes by file name. Panics with a clear
/// message if missing.
pub fn load_sample(name: &str) -> Vec<u8> {
    let path = samples_dir().join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read sample {}: {e}", path.display()))
}

/// The five synthetic fixtures and six anonymized samples, each with the
/// delimiters to tokenize it (the ISA-less fragment gets caller delimiters).
pub fn all_files() -> Vec<(String, Vec<u8>, edi835_core::Delimiters)> {
    let fixtures = [
        "emedny_sample.txt",
        "united_healthcare_legacy_sample.txt",
        "multi_claim_sample.txt",
        "trizetto_sample.rmt",
    ];
    let samples = [
        "edi835_test_davisvision.RMT",
        "edi835_test_eyemed.RMT",
        "edi835_test_file.RMT",
        "edi835_test_not_available_claim_id.RMT",
        "edi835_test_united.rmt",
        "edi835_test_versant.RMT",
    ];
    let mut files = Vec::new();
    for name in fixtures {
        let bytes = load_fixture(name);
        let delims = edi835_core::Delimiters::from_isa(&bytes).expect(name);
        files.push((name.to_string(), bytes, delims));
    }
    let blue = load_fixture("blue_cross_nc_sample.txt");
    files.push((
        "blue_cross_nc_sample.txt".to_string(),
        blue,
        edi835_core::Delimiters::new(b'*', b':', b'~'),
    ));
    for name in samples {
        let bytes = load_sample(name);
        let delims = edi835_core::Delimiters::from_isa(&bytes).expect(name);
        files.push((name.to_string(), bytes, delims));
    }
    files
}

/// Feed every segment to a fresh engine, then `finish`, and return every event and the engine.
pub fn run_engine_keeping<'s, 'a>(
    spec: &'s edi835_core::Spec,
    segments: impl IntoIterator<Item = edi835_core::Segment<'a>>,
) -> (Vec<edi835_core::Event>, edi835_core::LoopEngine<'s>) {
    let mut engine = edi835_core::LoopEngine::new(spec);
    let mut events = Vec::new();
    for segment in segments {
        events.extend_from_slice(engine.feed(&segment));
    }
    events.extend_from_slice(engine.finish());
    (events, engine)
}

/// Feed every segment to a fresh engine, then `finish`, and return every event in order.
pub fn run_engine<'a>(
    spec: &edi835_core::Spec,
    segments: impl IntoIterator<Item = edi835_core::Segment<'a>>,
) -> Vec<edi835_core::Event> {
    run_engine_keeping(spec, segments).0
}

/// Tokenize `bytes` with `delims` and run the engine over the result.
pub fn events_of(
    spec: &edi835_core::Spec,
    bytes: &[u8],
    delims: edi835_core::Delimiters,
) -> Vec<edi835_core::Event> {
    run_engine(spec, edi835_core::Tokenizer::with_delimiters(bytes, delims))
}

/// Tokenize `bytes` with `delims`, run the engine and the envelope checker
/// side by side, and return every diagnostic in order, `finish` included.
pub fn diagnostics_of(
    spec: &edi835_core::Spec,
    bytes: &[u8],
    delims: edi835_core::Delimiters,
) -> Vec<edi835_core::Diagnostic> {
    let mut engine = edi835_core::LoopEngine::new(spec);
    let mut checker = edi835_core::EnvelopeChecker::new(spec);
    let mut diagnostics = Vec::new();
    for segment in edi835_core::Tokenizer::with_delimiters(bytes, delims) {
        let events = engine.feed(&segment);
        diagnostics.extend_from_slice(checker.on(&segment, events));
    }
    engine.finish();
    diagnostics.extend_from_slice(checker.finish());
    diagnostics
}

/// A table's header line: every column as `name: type`, separated by ` | `.
pub fn table_header(table: &edi835_core::Table) -> String {
    table
        .columns()
        .iter()
        .map(|(name, column)| format!("{name}: {}", column.kind()))
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Every row of a table as one line, cells rendered by
/// `ColumnData::render` and separated by ` | `.
pub fn table_rows(table: &edi835_core::Table) -> Vec<String> {
    (0..table.len())
        .map(|row| {
            table
                .columns()
                .iter()
                .map(|(_, column)| column.render(row).unwrap_or_default())
                .collect::<Vec<_>>()
                .join(" | ")
        })
        .collect()
}
