//! Event streams compared against committed golden files. Small files keep
//! the full stream; the two large samples keep a count summary. Regenerate
//! with `UPDATE_GOLDEN=1 cargo test --test engine_golden`, inspect the diff,
//! commit.

mod common;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use edi835_core::{Event, Spec, Tokenizer};

const SUMMARY_ONLY: &[&str] = &["edi835_test_united.rmt", "edi835_test_versant.RMT"];

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

/// Human-readable location of the first difference between two line streams.
fn describe_diff(actual: &str, expected: &str) -> String {
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
    let segments: Vec<_> = Tokenizer::with_delimiters(bytes, delims).collect();
    let events = common::run_engine(spec, segments.iter().cloned());
    for event in events {
        let segment_id = match event {
            edi835_core::Event::Captured { segment, .. }
            | edi835_core::Event::Unmatched { segment }
            | edi835_core::Event::Empty { segment } => {
                segments.get(segment).map(|s| s.id).unwrap_or(b"")
            }
            _ => b"",
        };
        let _ = writeln!(out, "{}", line(spec, event, segment_id));
    }
    out
}

fn summary(spec: &Spec, bytes: &[u8], delims: edi835_core::Delimiters) -> String {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let segments: Vec<_> = Tokenizer::with_delimiters(bytes, delims).collect();
    let events = common::run_engine(spec, segments.iter().cloned());
    for event in events {
        let segment_id = match event {
            edi835_core::Event::Captured { segment, .. }
            | edi835_core::Event::Unmatched { segment }
            | edi835_core::Event::Empty { segment } => {
                segments.get(segment).map(|s| s.id).unwrap_or(b"")
            }
            _ => b"",
        };
        let key = line(spec, event, segment_id);
        let key = key
            .rsplit_once(" #")
            .map(|(k, _)| k.to_string())
            .unwrap_or(key);
        *counts.entry(key).or_default() += 1;
    }
    counts.into_iter().fold(String::new(), |mut out, (key, n)| {
        let _ = writeln!(out, "{key} x{n}");
        out
    })
}

#[test]
fn event_streams_match_the_golden_files() {
    use std::collections::BTreeSet;
    let spec = Spec::builtin_835();
    let update = std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1");
    let mut failures = Vec::new();
    let mut expected_paths = BTreeSet::new();
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
        expected_paths.insert(path.clone());
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
            failures.push(format!(
                "{name} ({}): {}",
                path.display(),
                describe_diff(&actual, &expected)
            ));
        }
    }
    if let Ok(entries) = std::fs::read_dir(golden_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && !expected_paths.contains(&path) {
                failures.push(format!(
                    "orphaned golden file with no test that compares it: {}",
                    path.display()
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn describe_diff_reports_content_drift() {
    let actual = "line 1\nline 2a\nline 3";
    let expected = "line 1\nline 2b\nline 3";
    let diff = describe_diff(actual, expected);
    assert_eq!(diff, r#"line 2: actual "line 2a", expected "line 2b""#);
}

#[test]
fn describe_diff_reports_when_actual_is_longer() {
    let actual = "line 1\nline 2\nline 3 extra";
    let expected = "line 1\nline 2";
    let diff = describe_diff(actual, expected);
    assert_eq!(
        diff,
        r#"expected ends at line 2; actual has 1 extra line(s), first: "line 3 extra""#
    );
}

#[test]
fn describe_diff_reports_when_expected_is_longer() {
    let actual = "line 1\nline 2";
    let expected = "line 1\nline 2\nline 3 missing";
    let diff = describe_diff(actual, expected);
    assert_eq!(
        diff,
        r#"actual ends at line 2; expected has 1 more line(s), first: "line 3 missing""#
    );
}
