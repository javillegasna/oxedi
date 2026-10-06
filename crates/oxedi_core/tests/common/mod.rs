#![allow(dead_code)] // each test binary compiles this module; not all use every helper
//! Shared helpers for integration tests. Lives in `tests/common/mod.rs` so Cargo
//! treats it as a module (not its own test binary) when included via `mod common;`.

use std::path::{Path, PathBuf};

/// The two large samples, whose goldens keep a summary instead of every line.
pub const SUMMARY_ONLY: &[&str] = &["edi835_test_united.rmt", "edi835_test_versant.RMT"];

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
pub fn all_files() -> Vec<(String, Vec<u8>, oxedi_core::Delimiters)> {
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
        let delims = oxedi_core::Delimiters::from_isa(&bytes).expect(name);
        files.push((name.to_string(), bytes, delims));
    }
    let blue = load_fixture("blue_cross_nc_sample.txt");
    files.push((
        "blue_cross_nc_sample.txt".to_string(),
        blue,
        oxedi_core::Delimiters::new(b'*', b':', b'~'),
    ));
    for name in samples {
        let bytes = load_sample(name);
        let delims = oxedi_core::Delimiters::from_isa(&bytes).expect(name);
        files.push((name.to_string(), bytes, delims));
    }
    files
}

/// The 5010 and 4010 built-in specs, as candidates for [`select`].
pub fn builtins() -> (oxedi_core::Spec, oxedi_core::Spec) {
    (
        oxedi_core::Spec::builtin_835(),
        oxedi_core::Spec::builtin_835_4010(),
    )
}

/// The built-in spec of the version the file declares, 5010 when it
/// declares none of theirs: what a caller that passes no spec gets.
pub fn select<'a>(
    five: &'a oxedi_core::Spec,
    four: &'a oxedi_core::Spec,
    bytes: &[u8],
    delims: oxedi_core::Delimiters,
) -> &'a oxedi_core::Spec {
    oxedi_core::Spec::select(
        &[five, four],
        five,
        oxedi_core::Tokenizer::with_delimiters(bytes, delims),
    )
}

/// Feed every segment to a fresh engine, then `finish`, and return every event and the engine.
pub fn run_engine_keeping<'s, 'a>(
    spec: &'s oxedi_core::Spec,
    segments: impl IntoIterator<Item = oxedi_core::Segment<'a>>,
) -> (Vec<oxedi_core::Event>, oxedi_core::LoopEngine<'s>) {
    let mut engine = oxedi_core::LoopEngine::new(spec);
    let mut events = Vec::new();
    for segment in segments {
        events.extend_from_slice(engine.feed(&segment));
    }
    events.extend_from_slice(engine.finish());
    (events, engine)
}

/// Feed every segment to a fresh engine, then `finish`, and return every event in order.
pub fn run_engine<'a>(
    spec: &oxedi_core::Spec,
    segments: impl IntoIterator<Item = oxedi_core::Segment<'a>>,
) -> Vec<oxedi_core::Event> {
    run_engine_keeping(spec, segments).0
}

/// Tokenize `bytes` with `delims` and run the engine over the result.
pub fn events_of(
    spec: &oxedi_core::Spec,
    bytes: &[u8],
    delims: oxedi_core::Delimiters,
) -> Vec<oxedi_core::Event> {
    run_engine(spec, oxedi_core::Tokenizer::with_delimiters(bytes, delims))
}

/// Tokenize `bytes` with `delims`, run the engine and the envelope checker
/// side by side, and return every diagnostic in order, `finish` included.
pub fn diagnostics_of(
    spec: &oxedi_core::Spec,
    bytes: &[u8],
    delims: oxedi_core::Delimiters,
) -> Vec<oxedi_core::Diagnostic> {
    let mut engine = oxedi_core::LoopEngine::new(spec);
    let mut checker = oxedi_core::EnvelopeChecker::new(spec, &delims);
    let mut diagnostics = Vec::new();
    for segment in oxedi_core::Tokenizer::with_delimiters(bytes, delims) {
        let events = engine.feed(&segment);
        diagnostics.extend_from_slice(checker.on(&segment, events));
    }
    engine.finish();
    diagnostics.extend_from_slice(checker.finish());
    diagnostics
}

/// Human-readable location of the first difference between two line streams.
pub fn describe_diff(actual: &str, expected: &str) -> String {
    let actual_lines: Vec<&str> = actual.lines().collect();
    let expected_lines: Vec<&str> = expected.lines().collect();

    for (i, (a, e)) in actual_lines.iter().zip(expected_lines.iter()).enumerate() {
        if a != e {
            return format!("line {}: actual {:?}, expected {:?}", i + 1, a, e);
        }
    }

    match actual_lines.len().cmp(&expected_lines.len()) {
        std::cmp::Ordering::Greater => {
            let extra = actual_lines.len() - expected_lines.len();
            let first_extra = actual_lines[expected_lines.len()];
            format!(
                "expected ends at line {}; actual has {} extra line(s), first: {:?}",
                expected_lines.len(),
                extra,
                first_extra
            )
        }
        std::cmp::Ordering::Less => {
            let extra = expected_lines.len() - actual_lines.len();
            let first_extra = expected_lines[actual_lines.len()];
            format!(
                "actual ends at line {}; expected has {} more line(s), first: {:?}",
                actual_lines.len(),
                extra,
                first_extra
            )
        }
        std::cmp::Ordering::Equal => "no difference".to_string(),
    }
}

/// Compares each `(path, actual)` pair with the committed file at `path`, or
/// writes `actual` there when `UPDATE_GOLDEN=1`, then reports every file
/// directly in `dir` that no pair names and every subdirectory of `dir` not
/// listed in `allowed_subdirs`. Returns one message per failure.
pub fn compare_goldens(
    dir: &Path,
    outputs: &[(PathBuf, String)],
    allowed_subdirs: &[&str],
) -> Vec<String> {
    let update = std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1");
    let mut failures = Vec::new();
    for (path, actual) in outputs {
        if update {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(path, actual).unwrap();
            continue;
        }
        let expected = std::fs::read_to_string(path).unwrap_or_else(|e| {
            panic!(
                "{}: {e}; run with UPDATE_GOLDEN=1 to create it",
                path.display()
            )
        });
        if *actual != expected {
            failures.push(format!(
                "{}: {}",
                path.display(),
                describe_diff(actual, &expected)
            ));
        }
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && !outputs.iter().any(|(named, _)| *named == path) {
                failures.push(format!(
                    "orphaned golden file with no test that compares it: {}",
                    path.display()
                ));
            } else if path.is_dir()
                && !allowed_subdirs
                    .iter()
                    .any(|name| entry.file_name() == *name)
            {
                failures.push(format!(
                    "unexpected golden subdirectory with no test that compares it: {}",
                    path.display()
                ));
            }
        }
    }
    failures
}

#[test]
fn compare_goldens_reports_orphan_files_and_unlisted_subdirectories() {
    let dir = std::env::temp_dir().join(format!("edi835_goldens_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("project")).unwrap();
    std::fs::create_dir_all(dir.join("stray")).unwrap();
    std::fs::write(dir.join("orphan.txt"), "x").unwrap();
    let failures = compare_goldens(&dir, &[], &["project"]);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(failures.len(), 2, "{failures:#?}");
    assert!(failures.iter().any(|f| f.contains("orphan.txt")));
    assert!(failures.iter().any(|f| f.contains("stray")));
}
