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

#[test]
fn crlf_and_lf_line_ends_are_equal() {
    assert_eq!(first_difference("a\r\nb", "a\nb"), None);
}

#[test]
fn two_empty_texts_are_equal() {
    assert_eq!(first_difference("", ""), None);
}

#[test]
fn an_empty_text_lacks_the_empty_line_a_lone_break_holds() {
    assert_eq!(
        first_difference("", "\n"),
        Some(LineDifference {
            line: 1,
            expected: Line::End,
            actual: text(""),
        })
    );
}

#[test]
fn multi_byte_lines_are_kept_whole() {
    assert_eq!(
        first_difference("año\nñandú", "año\nñandá"),
        Some(LineDifference {
            line: 2,
            expected: text("ñandú"),
            actual: text("ñandá"),
        })
    );
}
