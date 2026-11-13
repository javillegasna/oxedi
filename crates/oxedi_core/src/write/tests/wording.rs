//! The text of the writer's message pieces.

use crate::write::ValueOrigin;
use crate::write::wording::{
    Counted, Matches, Numbered, ReadBackOrigin, Reference, Source, WrittenBy,
};

#[test]
fn every_piece_says_its_branches_in_full() {
    let source = |column| Source("claims", column).to_string();
    let reference = |value| Reference(value, "claims").to_string();
    let count = |count| Counted(count, "finding", "findings").to_string();
    let row = ValueOrigin::Row {
        table: "claims".into(),
        row: 3,
    };
    let two = ["a".to_string(), "b".to_string()];
    let cases = [
        (source(Some("status")), "table \"claims\" column \"status\""),
        (source(None), "table \"claims\" anchor segment"),
        (
            WrittenBy(Some("claims")).to_string(),
            ", written by table \"claims\",",
        ),
        (WrittenBy(None).to_string(), ", which no table writes,"),
        (Matches(&[]).to_string(), "matches none of its occurrences"),
        (
            Matches(&two).to_string(),
            "matches 2 of its occurrences (\"a\", \"b\"); name one with \"occurrence\"",
        ),
        (
            reference(Some(7)),
            "refers to row 7, which table \"claims\" lacks",
        ),
        (reference(None), "is null"),
        (
            ReadBackOrigin(Some(&row)).to_string(),
            "table \"claims\" row 3",
        ),
        (ReadBackOrigin(None).to_string(), "the written file"),
        (count(1), "1 finding"),
        (count(0), "0 findings"),
        (count(2), "2 findings"),
        (Numbered(&["a", "b"]).to_string(), "\n1. a\n2. b"),
        (Numbered::<&str>(&[]).to_string(), ""),
    ];
    for (text, expected) in cases {
        assert_eq!(text, expected);
    }
}
