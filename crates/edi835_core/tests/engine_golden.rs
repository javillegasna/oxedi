//! Event streams compared against committed golden files. Small files keep
//! the full stream; the two large samples keep a count summary. Regenerate
//! with `UPDATE_GOLDEN=1 cargo test --test engine_golden`, inspect the diff,
//! commit.

mod common;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use edi835_core::{Event, LoopEngine, Spec, Tokenizer};

const SUMMARY_ONLY: &[&str] = &["edi835_test_united.rmt", "edi835_test_versant.RMT"];

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn line(spec: &Spec, event: Event, segment_id: &[u8]) -> String {
    let id = String::from_utf8_lossy(segment_id);
    match event {
        Event::LoopOpened { id: l, implicit } => format!(
            "open{} {}",
            if implicit { "!" } else { "" },
            spec.loop_name(l)
        ),
        Event::LoopClosed { id: l } => format!("close {}", spec.loop_name(l)),
        Event::Captured { id: l, segment } => format!("cap {} {id} #{segment}", spec.loop_name(l)),
        Event::Unmatched { segment } => format!("unmatched {id} #{segment}"),
        Event::Empty { segment } => format!("empty #{segment}"),
    }
}

fn full_stream(spec: &Spec, bytes: &[u8], delims: edi835_core::Delimiters) -> String {
    let mut out = String::new();
    let mut engine = LoopEngine::new(spec);
    for segment in Tokenizer::with_delimiters(bytes, delims) {
        for &event in engine.feed(&segment) {
            let _ = writeln!(out, "{}", line(spec, event, segment.id));
        }
    }
    for &event in engine.finish() {
        let _ = writeln!(out, "{}", line(spec, event, b""));
    }
    out
}

fn summary(spec: &Spec, bytes: &[u8], delims: edi835_core::Delimiters) -> String {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut engine = LoopEngine::new(spec);
    for segment in Tokenizer::with_delimiters(bytes, delims) {
        for &event in engine.feed(&segment) {
            let key = line(spec, event, segment.id);
            let key = key
                .rsplit_once(" #")
                .map(|(k, _)| k.to_string())
                .unwrap_or(key);
            *counts.entry(key).or_default() += 1;
        }
    }
    for &event in engine.finish() {
        *counts.entry(line(spec, event, b"")).or_default() += 1;
    }
    counts.into_iter().fold(String::new(), |mut out, (key, n)| {
        let _ = writeln!(out, "{key} x{n}");
        out
    })
}

#[test]
fn event_streams_match_the_golden_files() {
    let spec = Spec::builtin_835();
    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    let mut failures = Vec::new();
    for (name, bytes, delims) in common::all_files() {
        let (actual, path) = if SUMMARY_ONLY.contains(&name.as_str()) {
            (
                summary(&spec, &bytes, delims),
                golden_dir().join(format!("{name}.summary.txt")),
            )
        } else {
            (
                full_stream(&spec, &bytes, delims),
                golden_dir().join(format!("{name}.events.txt")),
            )
        };
        if update {
            std::fs::create_dir_all(golden_dir()).unwrap();
            std::fs::write(&path, &actual).unwrap();
            continue;
        }
        let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "{}: {e}; run with UPDATE_GOLDEN=1 to create it",
                path.display()
            )
        });
        if actual != expected {
            let first_diff = actual
                .lines()
                .zip(expected.lines())
                .position(|(a, e)| a != e);
            failures.push(format!(
                "{name}: differs at line {:?}",
                first_diff.map(|n| n + 1)
            ));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
