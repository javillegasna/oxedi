//! What each refusal says to do.

use std::collections::HashSet;

use crate::write::Refusal;

fn s(text: &str) -> String {
    text.to_string()
}

#[test]
fn every_refusal_says_what_to_do() {
    let cases = vec![
        (
            Refusal::RequiredLoopWithoutSource {
                loop_name: s("2100"),
                parent: s("2000"),
                trigger: s("\"NM1\""),
            },
            "anchor a table on the loop or add a column that reads it, or make the loop \
             situational in the guide",
        ),
        (
            Refusal::RequiredOccurrenceWithoutSource {
                loop_name: s("2100"),
                occurrence: s("PER"),
                selector: s("\"PER\""),
            },
            "add a column that writes the occurrence, or make it situational in the guide",
        ),
        (
            Refusal::RequiredElementWithoutSource {
                table: None,
                loop_name: s("2100"),
                occurrence: s("NM1"),
                place: s("NM102"),
                name: s("Entity Type"),
            },
            "add a column that writes the element, give it a single code, or make it \
             situational in the guide",
        ),
        (
            Refusal::AmbiguousSource {
                table: s("t"),
                column: None,
                loop_name: s("2100"),
                source: s("\"DTM\""),
                candidates: vec![],
            },
            "anchor the table on a segment that occurs once in the loop",
        ),
        (
            Refusal::AmbiguousSource {
                table: s("t"),
                column: Some(s("c")),
                loop_name: s("2100"),
                source: s("\"DTM\""),
                candidates: vec![],
            },
            "name an occurrence the loop has with \"occurrence\", or give the column a \
             \"where\" that matches exactly one",
        ),
        (
            Refusal::PickNotContiguous {
                table: s("t"),
                column: s("c"),
                loop_name: s("2100"),
                occurrence: s("DTM"),
                picks: vec![s("2")],
            },
            "renumber the picks of the occurrence's columns from 1 with none missing, or keep \
             a lone \"last\"",
        ),
        (
            Refusal::WrittenByAnotherTable {
                table: s("t"),
                column: s("c"),
                loop_name: s("2100"),
                writer: s("table \"u\""),
            },
            "remove the column, or move it to the table that writes the loop, when a table does",
        ),
        (
            Refusal::DuplicateSource {
                table: s("t"),
                column: s("c"),
                other: s("d"),
                loop_name: s("2100"),
                occurrence: s("CLP"),
                place: s("CLP01"),
            },
            "remove one of the two columns",
        ),
        (
            Refusal::EnvelopeColumn {
                table: s("t"),
                column: s("c"),
                loop_name: s("ISA"),
                occurrence: s("ISA"),
            },
            "remove the column; the writer takes the envelope's values from the caller's \
             envelope",
        ),
        (
            Refusal::EnvelopeWithoutOccurrence {
                loop_name: s("interchange"),
                trigger: s("\"ISA\" with no conditions"),
            },
            "declare the occurrence the loop's trigger opens on",
        ),
        (
            Refusal::AnchoredByAnotherTable {
                table: s("t"),
                loop_name: s("2100"),
                other: s("u"),
            },
            "anchor one of the two tables on a segment of the loop, or remove one of them",
        ),
        (
            Refusal::SeveralAnchorLoops {
                table: s("t"),
                loops: vec![s("1000A"), s("1000B")],
            },
            "anchor the table on one loop",
        ),
        (
            Refusal::RepeatWithoutInstances {
                table: s("t"),
                loop_name: s("2100"),
                segment: s("\"CAS\""),
            },
            "anchor a table on the loop or add a column that reads it",
        ),
        (
            Refusal::CodeOutsideList {
                table: s("t"),
                column: None,
                loop_name: s("2100"),
                occurrence: s("PER"),
                place: s("PER01"),
                value: s("ZZ"),
                codes: vec![s("BL")],
            },
            "change the fixed code to one the list allows, or add it to the code list in the \
             guide",
        ),
    ];
    assert_eq!(cases.len(), 14);
    for (refusal, expected) in &cases {
        assert_eq!(refusal.remedy(), *expected, "{refusal}");
    }
    let distinct: HashSet<&str> = cases.iter().map(|(refusal, _)| refusal.remedy()).collect();
    assert_eq!(distinct.len(), 14);
}
