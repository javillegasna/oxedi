use super::*;
use crate::diagnostic::{Diagnostic, PARSER_ORIGIN};

fn unknown_segment() -> Diagnostic {
    Diagnostic::new(
        Rule::UnknownSegment {
            segment_id: b"XX".to_vec(),
        },
        Some(3),
        None,
        Some(2),
        vec![],
        b"XX".to_vec(),
    )
}

#[test]
fn a_diagnostic_record_holds_its_rule_and_place() {
    let d = unknown_segment();
    assert_eq!(
        d.record(),
        ProblemRecord {
            kind: "UnknownSegment",
            place: Place::File {
                segment: Some(3),
                element: None,
                component: Some(2),
                path: &[],
            },
            text: d.to_string(),
            detail: Detail::Diagnostic {
                level: SnipLevel::L1,
                rule: &d.rule,
                datum: b"XX",
                origin: PARSER_ORIGIN,
                code: None,
            },
        }
    );
}
