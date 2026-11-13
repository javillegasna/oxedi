//! The text of the writer's message pieces.

use crate::write::ValueOrigin;
use crate::write::wording::{
    Counted, Matches, Numbered, ReadBackOrigin, Reference, Source, WrittenBy,
};

#[test]
fn a_source_with_a_column_names_the_table_and_the_column() {
    let source = Source {
        table: "claims",
        column: Some("status"),
    };
    assert_eq!(source.to_string(), "table \"claims\" column \"status\"");
}

#[test]
fn a_source_without_a_column_names_the_anchor_segment() {
    let source = Source {
        table: "claims",
        column: None,
    };
    assert_eq!(source.to_string(), "table \"claims\" anchor segment");
}

#[test]
fn a_writing_table_is_named() {
    assert_eq!(
        WrittenBy(Some("claims")).to_string(),
        ", written by table \"claims\","
    );
}

#[test]
fn no_writing_table_is_said_so() {
    assert_eq!(WrittenBy(None).to_string(), ", which no table writes,");
}

#[test]
fn no_candidates_match_none_of_the_occurrences() {
    assert_eq!(Matches(&[]).to_string(), "matches none of its occurrences");
}

#[test]
fn candidates_are_counted_and_named() {
    let candidates = ["a".to_string(), "b".to_string()];
    assert_eq!(
        Matches(&candidates).to_string(),
        "matches 2 of its occurrences (\"a\", \"b\"); name one with \"occurrence\""
    );
}

#[test]
fn a_reference_to_a_missing_row_names_the_row_and_the_parent() {
    let reference = Reference {
        value: Some(7),
        parent: "claims",
    };
    assert_eq!(
        reference.to_string(),
        "refers to row 7, which table \"claims\" lacks"
    );
}

#[test]
fn a_null_reference_is_said_so() {
    let reference = Reference {
        value: None,
        parent: "claims",
    };
    assert_eq!(reference.to_string(), "is null");
}

#[test]
fn a_read_back_origin_names_the_origin_or_the_written_file() {
    let origin = ValueOrigin::Row {
        table: "claims".into(),
        row: 3,
    };
    assert_eq!(
        ReadBackOrigin(Some(&origin)).to_string(),
        "table \"claims\" row 3"
    );
    assert_eq!(ReadBackOrigin(None).to_string(), "the written file");
}

#[test]
fn a_count_takes_the_singular_only_for_one() {
    let count = |count| Counted {
        count,
        one: "finding",
        many: "findings",
    };
    assert_eq!(count(1).to_string(), "1 finding");
    assert_eq!(count(0).to_string(), "0 findings");
    assert_eq!(count(2).to_string(), "2 findings");
}

#[test]
fn numbered_items_start_at_one_each_on_its_own_line() {
    assert_eq!(Numbered(&["a", "b"]).to_string(), "\n1. a\n2. b");
    assert_eq!(Numbered::<&str>(&[]).to_string(), "");
}
