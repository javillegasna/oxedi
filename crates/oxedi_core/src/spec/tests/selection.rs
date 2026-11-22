//! The choice of a spec by the version it declares.

use std::cell::Cell;

use crate::spec::Spec;
use crate::spec::test_support::{declaring, segs};

const ISA: &str = "ISA*00*          *00*          *ZZ*S              *ZZ*R              *240101*1200*^*00501*000000001*0*P*:~";

#[test]
fn select_picks_the_first_candidate_whose_declared_element_matches() {
    let five = declaring(
        "5010",
        r#"{"segment":"GS","element":8,"values":["005010X221A1"]}"#,
    );
    let four = declaring(
        "4010",
        r#"{"segment":"GS","element":8,"values":["004010X091A1","004010"]}"#,
    );
    let plain = declaring("plain", "");
    let candidates = [&plain, &five, &four];
    let pick = |gs08: &str| {
        let input = format!("{ISA}GS*HP*S*R*20240101*1200*1*X*{gs08}~ST*835*0001~");
        Spec::select(&candidates, &five, segs(input.as_bytes())).name()
    };
    assert_eq!(pick("004010X091A1"), "4010");
    assert_eq!(pick("004010"), "4010");
    assert_eq!(pick("005010X221A1"), "5010");
    // No candidate matches: the default.
    assert_eq!(pick("003070"), "5010");
    // A composite value is not one of the declared values.
    assert_eq!(pick("004010:X"), "5010");
    // No segment carries the declaration: the default.
    let no_group = format!("{ISA}ST*835*0001~");
    assert_eq!(
        Spec::select(&candidates, &plain, segs(no_group.as_bytes())).name(),
        "plain"
    );
}

#[test]
fn select_honours_candidate_order_when_declarations_name_different_segments() {
    let by_st = declaring("st", r#"{"segment":"ST","element":3,"values":["V"]}"#);
    let by_gs = declaring("gs", r#"{"segment":"GS","element":8,"values":["G"]}"#);
    let default = declaring("default", "");
    let input = format!("{ISA}GS*HP*S*R*20240101*1200*1*X*G~ST*835*0001*V~");
    let segments = segs(input.as_bytes());
    // `gs` resolves first in the stream, but `st` comes first in the list.
    assert_eq!(
        Spec::select(&[&by_st, &by_gs], &default, segments.clone()).name(),
        "st"
    );
    assert_eq!(
        Spec::select(&[&by_gs, &by_st], &default, segments).name(),
        "gs"
    );
}

#[test]
fn select_stops_reading_once_the_choice_is_settled() {
    let five = declaring("5010", r#"{"segment":"GS","element":8,"values":["A"]}"#);
    let four = declaring("4010", r#"{"segment":"GS","element":8,"values":["B"]}"#);
    let input = format!("{ISA}GS*HP*S*R*20240101*1200*1*X*B~ST*835*0001~SE*2*0001~GE*1*1~");
    let read = Cell::new(0);
    let segments = segs(input.as_bytes()).into_iter().inspect(|_| {
        read.set(read.get() + 1);
    });
    assert_eq!(
        Spec::select(&[&five, &four], &five, segments).name(),
        "4010"
    );
    assert_eq!(read.get(), 2, "only ISA and GS are read");
}
