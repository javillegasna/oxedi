//! The first line where two texts differ.

/// One side of a difference: a line, or the end of the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// The line's text, without its line break.
    Text(String),
    /// The text ended before this line.
    End,
}

impl Line {
    fn of(line: Option<&str>) -> Line {
        line.map_or(Line::End, |text| Line::Text(text.to_owned()))
    }
}

/// Where two texts first differ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineDifference {
    /// 1-based line number.
    pub line: usize,
    /// What the expected text has there.
    pub expected: Line,
    /// What the actual text has there.
    pub actual: Line,
}

/// The first line where `expected` and `actual` differ, or `None` when their
/// lines are equal. Texts compare by `str::lines`, so a final line break does
/// not count and `"a\n"` equals `"a"`. One pass; no line is kept but the one
/// that differs.
pub fn first_difference(expected: &str, actual: &str) -> Option<LineDifference> {
    let mut expected_lines = expected.lines();
    let mut actual_lines = actual.lines();
    for line in 1.. {
        match (expected_lines.next(), actual_lines.next()) {
            (None, None) => return None,
            (Some(e), Some(a)) if e == a => {}
            (e, a) => {
                return Some(LineDifference {
                    line,
                    expected: Line::of(e),
                    actual: Line::of(a),
                });
            }
        }
    }
    None
}
