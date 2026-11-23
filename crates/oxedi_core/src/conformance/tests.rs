use super::*;

fn text(line: &str) -> Line {
    Line::Text(line.to_owned())
}

#[test]
fn equal_texts_have_no_difference() {
    assert_eq!(first_difference("a\nb\n", "a\nb"), None);
}

#[test]
fn a_changed_line() {
    assert_eq!(
        first_difference("a\nb", "a\nc"),
        Some(LineDifference {
            line: 2,
            expected: text("b"),
            actual: text("c"),
        })
    );
}

#[test]
fn actual_is_longer() {
    assert_eq!(
        first_difference("a", "a\nz"),
        Some(LineDifference {
            line: 2,
            expected: Line::End,
            actual: text("z"),
        })
    );
}

#[test]
fn expected_is_longer() {
    assert_eq!(
        first_difference("a\nz", "a"),
        Some(LineDifference {
            line: 2,
            expected: text("z"),
            actual: Line::End,
        })
    );
}
