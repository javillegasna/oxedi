//! The origin a diagnostic about the written file gets.

use crate::diagnostic::{Diagnostic, Rule};
use crate::write::envelope::Field;
use crate::write::finding::Origin;
use crate::write::trace::{Src, Traces};
use crate::write::writer::origin_of;

/// An opener written from a payments row and a closer whose control number
/// the envelope wrote.
fn traces() -> Traces {
    let mut traces = Traces::default();
    traces.entry(2, None, Src::Field(Field::ControlNumber));
    traces.close(Some((0, 3)));
    traces.entry(2, None, Src::Field(Field::ControlNumber));
    traces.close(None);
    traces
}

fn names(table: usize, column: Option<usize>) -> String {
    match column {
        None => format!("table{table}"),
        Some(column) => format!("column{column}"),
    }
}

#[test]
fn a_control_number_mismatch_names_the_envelope_field_of_the_closer() {
    let diagnostic = Diagnostic::new(
        Rule::ControlNumberMismatch {
            opener: b"ST".to_vec(),
            opener_element: 2,
            closer: b"SE".to_vec(),
            closer_element: 2,
            opener_value: b"0001".to_vec(),
            closer_value: b"0002".to_vec(),
            opened_at: Some(0),
        },
        Some(1),
        Some(2),
        None,
        Vec::new(),
        b"0002".to_vec(),
    );
    assert_eq!(
        origin_of(&diagnostic, &traces(), &names),
        Some(Origin::Envelope {
            field: "control_number".to_string()
        })
    );
}

#[test]
fn a_rule_about_a_whole_instance_names_the_row_that_opened_it() {
    let diagnostic = Diagnostic::new(
        Rule::UnterminatedLoop {
            loop_name: "transaction".to_string(),
            expected_end: b"SE".to_vec(),
            opened_at: Some(0),
        },
        None,
        None,
        None,
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(
        origin_of(&diagnostic, &traces(), &names),
        Some(Origin::Row {
            table: "table0".to_string(),
            row: 3
        })
    );
}
