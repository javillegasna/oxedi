//! The text of the writer's message pieces.

use crate::write::wording::{Matches, Source, WrittenBy};

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
