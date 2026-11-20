use crate::column::Cell;
use crate::value::{ColumnType, ElementTextError};

use super::super::render::{Part, cell_text, write_segment};
use crate::frame::SegmentEnd;
use crate::segment::Syntax;

#[test]
fn a_decimal_cell_takes_its_scale_from_a_decimal_column_only() {
    let mut out = Vec::new();
    let decimal = ColumnType::Decimal128 {
        precision: 38,
        scale: 2,
    };
    assert_eq!(
        cell_text(Cell::Decimal128(1250), decimal, || None, &mut out),
        Ok(true)
    );
    assert_eq!(out, b"12.5");
    let integer = ColumnType::Int64 { scale: 0 };
    assert_eq!(
        cell_text(Cell::Decimal128(1250), integer, || None, &mut out),
        Err(ElementTextError::NoDecimalScale { kind: integer })
    );
    assert_eq!(out, b"12.5");
}

fn part(element: usize, component: Option<usize>, text: Option<std::ops::Range<usize>>) -> Part {
    Part {
        element,
        component,
        text,
    }
}

fn written(parts: &[Part], texts: &[u8], line_break: bool) -> String {
    let syntax = Syntax {
        element: b'*',
        component: b':',
        end: SegmentEnd::new(b'~', None),
    };
    let mut out = Vec::new();
    write_segment(b"NM1", parts, texts, syntax, line_break, &mut out);
    String::from_utf8_lossy(&out).into_owned()
}

#[test]
fn a_segment_keeps_gaps_and_drops_trailing_empty_places() {
    let parts = [
        part(1, None, Some(0..2)),
        part(3, Some(2), Some(2..3)),
        part(4, None, None),
        part(5, Some(1), Some(3..3)),
    ];
    assert_eq!(written(&parts, b"ILB", true), "NM1*IL**:B~\n");
}

#[test]
fn a_second_part_for_one_place_is_ignored_and_holds_no_trailing_separator_open() {
    let parts = [
        part(1, None, Some(0..1)),
        part(1, None, Some(1..2)),
        part(2, None, None),
        part(2, None, Some(1..2)),
    ];
    assert_eq!(written(&parts, b"AB", false), "NM1*A~");
}
