//! Occurrence checks over a small spec: an envelope `env` whose child `head`
//! numbers its own positions, and two children of `head` that share its
//! position space.

use super::tests::check_all;
use super::*;
use crate::{Delimiters, SnipLevel};

const SPEC: &str = r#"{"name":"o",
    "loops":{
        "env":{"trigger":{"segment":"EV"},"end":"EE","max":1,"occurrences":{
            "env":{"segment":"EV","pos":1,"usage":"required","max":1},
            "note":{"segment":"NT","pos":2}
        }},
        "head":{"parent":"env","trigger":{"segment":"HD"},"end":"TR","usage":"required","occurrences":{
            "hd":{"segment":"HD","pos":1,"usage":"required","max":1},
            "nt":{"segment":"NT","pos":20,"max":2},
            "rf_a":{"segment":"RF","pos":30,"usage":"required","max":1,"qualifier":{"element":1,"codes":["A"]}},
            "rf_b":{"segment":"RF","pos":30,"qualifier":{"element":1,"codes":["B","C"]}},
            "sm":{"segment":"SM","pos":90}
        }},
        "item":{"parent":"head","trigger":{"segment":"IT"},"usage":"required","max":2,"occurrences":{
            "it":{"segment":"IT","pos":40,"usage":"required"},
            "qt":{"segment":"QT","pos":50,"usage":"required"}
        }},
        "opt":{"parent":"head","trigger":{"segment":"OP"},"occurrences":{
            "op":{"segment":"OP","pos":60}
        }}
    },
    "segments":{"RF":{"elements":{"1":{"name":"kind","type":"ID"}}}}
}"#;

fn spec() -> Spec {
    Spec::from_json(SPEC).unwrap()
}

fn rendered(input: &str) -> Vec<String> {
    check_all(&spec(), input)
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[test]
fn a_complete_instance_in_order_yields_nothing() {
    assert_eq!(
        rendered("EV~NT~HD~NT~NT~RF*B~RF*A~RF*C~IT~QT~IT~QT~OP~SM~TR~EE~"),
        Vec::<String>::new()
    );
}

#[test]
fn a_child_that_numbers_its_own_positions_is_not_ordered_against_its_parent() {
    // `head` opens at position 1, which does not come after `env`'s first
    // position, so `NT` (position 2 in `env`) before it is not out of order.
    assert_eq!(rendered("EV~NT~HD~RF*A~IT~QT~TR~EE~"), Vec::<String>::new());
}

#[test]
fn a_missing_required_occurrence_is_reported_when_its_instance_closes() {
    assert_eq!(
        rendered("EV~HD~IT~QT~TR~EE~"),
        vec![
            "SNIP 2 · loop \"head\" opened at segment #1 closed without its required occurrence \"rf_a\" (\"RF\" where RF01 is \"A\") · segment #4 · at env#1/head#1 · datum \"TR\""
        ]
    );
    assert_eq!(
        rendered("EV~HD~RF*A~IT~TR~EE~"),
        vec![
            "SNIP 2 · loop \"item\" opened at segment #3 closed without its required occurrence \"qt\" (\"QT\") · segment #4 · at env#1/head#1/item#1 · datum \"TR\""
        ]
    );
}

#[test]
fn a_missing_required_child_loop_is_reported_when_its_parent_closes() {
    assert_eq!(
        rendered("EV~HD~RF*A~TR~EE~"),
        vec![
            "SNIP 2 · loop \"head\" opened at segment #1 closed without its required child loop \"item\" (trigger \"IT\" with no conditions) · segment #3 · at env#1/head#1 · datum \"TR\""
        ]
    );
    // At the end of the stream the finding names no segment.
    assert_eq!(
        rendered("EV~"),
        vec![
            "SNIP 1 · loop \"env\" opened at segment #0 closed without its end segment \"EE\" · end of stream · at env#1 · datum \"\"",
            "SNIP 2 · loop \"env\" opened at segment #0 closed without its required child loop \"head\" (trigger \"HD\" with no conditions) · end of stream · at env#1 · datum \"\"",
        ]
    );
}

#[test]
fn every_repeat_past_an_occurrence_maximum_is_reported() {
    assert_eq!(
        rendered("EV~HD~NT~NT~NT~NT~RF*A~IT~QT~TR~EE~"),
        vec![
            "SNIP 2 · occurrence \"nt\" (\"NT\") of loop \"head\" appears 3 times in one instance; the spec allows at most 2 · segment #4 · at env#1/head#1 · datum \"NT\"",
            "SNIP 2 · occurrence \"nt\" (\"NT\") of loop \"head\" appears 4 times in one instance; the spec allows at most 2 · segment #5 · at env#1/head#1 · datum \"NT\"",
        ]
    );
    // The counts are per instance.
    assert_eq!(
        rendered("EV~HD~NT~NT~RF*A~IT~QT~TR~HD~NT~NT~RF*A~IT~QT~TR~EE~"),
        Vec::<String>::new()
    );
}

#[test]
fn every_instance_past_a_loop_maximum_is_reported() {
    assert_eq!(
        rendered("EV~HD~RF*A~IT~QT~IT~QT~IT~QT~TR~EE~"),
        vec![
            "SNIP 2 · loop \"item\" has 3 instances under one instance of loop \"head\"; the spec allows at most 2 · segment #7 · at env#1/head#1/item#3 · datum \"IT\""
        ]
    );
    assert_eq!(
        rendered("EV~HD~RF*A~IT~QT~TR~EE~EV~HD~RF*A~IT~QT~TR~EE~"),
        vec![
            "SNIP 2 · loop \"env\" has 2 instances at the root; the spec allows at most 1 · segment #7 · at env#2 · datum \"EV\""
        ]
    );
}

#[test]
fn a_segment_after_a_higher_position_is_out_of_order() {
    assert_eq!(
        rendered("EV~HD~RF*A~NT~IT~QT~TR~EE~"),
        vec![
            "SNIP 2 · occurrence \"nt\" (position 20) of loop \"head\" comes after occurrence \"rf_a\" (position 30) of loop \"head\" · segment #3 · at env#1/head#1 · datum \"NT\""
        ]
    );
    // A child loop is placed among its parent's occurrences by its trigger,
    // and a parent occurrence after a child loop is compared with that child.
    assert_eq!(
        rendered("EV~HD~RF*A~OP~IT~QT~SM~RF*B~TR~EE~"),
        vec![
            "SNIP 2 · occurrence \"it\" (position 40) of loop \"item\" comes after occurrence \"op\" (position 60) of loop \"opt\" · segment #4 · at env#1/head#1/item#1 · datum \"IT\"",
            "SNIP 2 · occurrence \"rf_b\" (position 30) of loop \"head\" comes after occurrence \"sm\" (position 90) of loop \"head\" · segment #7 · at env#1/head#1 · datum \"RF\"",
        ]
    );
}

#[test]
fn a_segment_whose_qualifier_matches_no_occurrence_is_reported_and_kept() {
    let diagnostics = check_all(&spec(), "EV~HD~RF*Z~RF~RF*A~IT~QT~TR~EE~");
    let lines: Vec<String> = diagnostics.iter().map(ToString::to_string).collect();
    assert_eq!(
        lines,
        vec![
            "SNIP 2 · segment \"RF\" matches none of the 2 occurrences loop \"head\" declares for it (\"rf_a\", \"rf_b\"): RF01 holds none of their qualifier codes · segment #2, element 1 · at env#1/head#1 · datum \"Z\"",
            "SNIP 2 · segment \"RF\" matches none of the 2 occurrences loop \"head\" declares for it (\"rf_a\", \"rf_b\"): RF01 holds none of their qualifier codes · segment #3, element 1 · at env#1/head#1 · datum \"\"",
        ]
    );
    assert_eq!(diagnostics[0].element, Some(1));
    // An unmatched segment counts toward no occurrence, so `rf_a` is missing.
    assert_eq!(
        check_all(&spec(), "EV~HD~RF*Z~IT~QT~TR~EE~")
            .iter()
            .map(|diagnostic| diagnostic.rule.kind())
            .collect::<Vec<_>>(),
        vec!["UnknownOccurrence", "RequiredOccurrenceMissing"]
    );
}

#[test]
fn an_implicit_instance_reports_nothing_it_lacks() {
    let diagnostics = check_all(&spec(), "EV~IT~QT~EE~");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.level == SnipLevel::L1),
        "{diagnostics:#?}"
    );
    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
}

#[test]
fn a_loop_without_occurrences_is_not_checked() {
    let spec = Spec::from_json(
        r#"{"name":"bare","loops":{
            "a":{"trigger":{"segment":"A"},"end":"Z"},
            "b":{"parent":"a","trigger":{"segment":"B"},"usage":"required","max":1}
        }}"#,
    )
    .unwrap();
    let kinds = |input: &str| -> Vec<&'static str> {
        check_all(&spec, input)
            .iter()
            .map(|diagnostic| diagnostic.rule.kind())
            .collect()
    };
    assert_eq!(kinds("A~B~Z~"), Vec::<&str>::new());
    // Its usage and maximum as a child still are.
    assert_eq!(kinds("A~Z~"), vec!["RequiredLoopMissing"]);
    assert_eq!(kinds("A~B~B~Z~"), vec!["LoopOverMax"]);
}

#[test]
fn finishing_restarts_the_root_counts() {
    let spec = spec();
    let delims = Delimiters::new(b'*', b':', b'~');
    let mut engine = crate::LoopEngine::new(&spec);
    let mut checker = EnvelopeChecker::new(&spec, &delims);
    for _ in 0..2 {
        let mut out = Vec::new();
        for segment in crate::Tokenizer::with_delimiters(&b"EV~HD~RF*A~IT~QT~TR~EE~"[..], delims) {
            let events = engine.feed(&segment);
            out.extend_from_slice(checker.on(&segment, events));
        }
        engine.finish();
        out.extend_from_slice(checker.finish());
        assert_eq!(out, Vec::new());
    }
}
