//! Event streams compared against committed golden files, each file under
//! the built-in spec of the version it declares. Small files keep
//! the full stream; the two large samples keep a count summary. Regenerate
//! with `UPDATE_GOLDEN=1 cargo test --test engine_golden`, inspect the diff,
//! commit. Goldens of other suites live in subdirectories, which this one
//! leaves alone.

mod common;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use edi835_core::{Event, Spec, Tokenizer};

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn line(spec: &Spec, event: Event, segment_id: &[u8]) -> String {
    let id = String::from_utf8_lossy(segment_id);
    match event {
        Event::LoopOpened {
            id: l,
            implicit,
            segment,
        } => format!(
            "open{} {} #{segment}",
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
    let (five, four) = common::builtins();
    let mut outputs = Vec::new();
    for (name, bytes, delims) in common::all_files() {
        let spec = common::select(&five, &four, &bytes, delims);
        outputs.push(if common::SUMMARY_ONLY.contains(&name.as_str()) {
            (
                golden_dir().join(format!("{name}.summary.txt")),
                summary(spec, &bytes, delims),
            )
        } else {
            (
                golden_dir().join(format!("{name}.events.txt")),
                full_stream(spec, &bytes, delims),
            )
        });
    }
    let failures = common::compare_goldens(&golden_dir(), &outputs, &["project"]);
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn describe_diff_reports_content_drift() {
    let actual = "line 1\nline 2a\nline 3";
    let expected = "line 1\nline 2b\nline 3";
    let diff = common::describe_diff(actual, expected);
    assert_eq!(diff, r#"line 2: actual "line 2a", expected "line 2b""#);
}

#[test]
fn describe_diff_reports_when_actual_is_longer() {
    let actual = "line 1\nline 2\nline 3 extra";
    let expected = "line 1\nline 2";
    let diff = common::describe_diff(actual, expected);
    assert_eq!(
        diff,
        r#"expected ends at line 2; actual has 1 extra line(s), first: "line 3 extra""#
    );
}

#[test]
fn describe_diff_reports_when_expected_is_longer() {
    let actual = "line 1\nline 2";
    let expected = "line 1\nline 2\nline 3 missing";
    let diff = common::describe_diff(actual, expected);
    assert_eq!(
        diff,
        r#"actual ends at line 2; expected has 1 more line(s), first: "line 3 missing""#
    );
}
