//! Values as element text, and segments as bytes.
//!
//! A segment is built from its parts, one per element or component, each
//! holding a value or nothing. A part with nothing and a part with empty text
//! both write an empty element; trailing empty elements, and trailing empty
//! components of a composite, are left out.

use std::io::Write as _;
use std::ops::Range;

use crate::column::{Cell, ColumnData, ColumnType, civil_from_days};

/// A date as `CCYYMMDD`, or `YYMMDD` when the element holds six bytes at
/// most; the reason when the date has no such text.
pub(super) fn date_text(days: i32, max: Option<usize>) -> Result<Vec<u8>, String> {
    if days.checked_add(719_468).is_none() {
        return Err("a date needs a year from 1 to 9999".to_string());
    }
    let (year, month, day) = civil_from_days(days);
    if max.is_some_and(|max| max < 8) {
        if !(1950..=2049).contains(&year) {
            return Err(format!(
                "the element holds a two-digit year, read as 1950 to 2049; the year is {year}"
            ));
        }
        return Ok(format!("{:02}{month:02}{day:02}", year % 100).into_bytes());
    }
    if !(1..=9999).contains(&year) {
        return Err("a date needs a year from 1 to 9999".to_string());
    }
    Ok(format!("{year:04}{month:02}{day:02}").into_bytes())
}

/// A time as `HHMM`, or `HHMMSS` when it has seconds and the element holds
/// them; the reason when the time has no such text.
pub(super) fn time_text(seconds: i32, max: Option<usize>) -> Result<Vec<u8>, String> {
    if !(0..86_400).contains(&seconds) {
        return Err("a time must fall within one day".to_string());
    }
    let (hours, minutes, rest) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if rest == 0 {
        return Ok(format!("{hours:02}{minutes:02}").into_bytes());
    }
    if max.is_some_and(|max| max < 6) {
        return Err("the element holds hours and minutes only".to_string());
    }
    Ok(format!("{hours:02}{minutes:02}{rest:02}").into_bytes())
}

/// A scaled decimal in fixed point, without trailing zeros in the fraction
/// and without a point when nothing follows it.
fn decimal_text(value: i128, scale: u8, out: &mut Vec<u8>) {
    let scale = usize::from(scale);
    if value < 0 {
        out.push(b'-');
    }
    // The digits, least significant first, at least one more than the scale.
    let mut digits = [0u8; 48];
    let mut count = 0;
    let mut rest = value.unsigned_abs();
    while (rest > 0 || count <= scale)
        && let Some(slot) = digits.get_mut(count)
    {
        *slot = b'0' + (rest % 10) as u8;
        rest /= 10;
        count += 1;
    }
    let digits = digits.get(..count).unwrap_or_default();
    let (fraction, whole) = digits.split_at(scale.min(digits.len()));
    out.extend(whole.iter().rev());
    let zeros = fraction.iter().take_while(|&&digit| digit == b'0').count();
    let kept = fraction.get(zeros..).unwrap_or_default();
    if !kept.is_empty() {
        out.push(b'.');
        out.extend(kept.iter().rev());
    }
}

/// Appends a cell's text for an element; `Ok(false)` for a null cell, the
/// reason when the value has no text the element can hold. `max` gives the
/// element's maximum length, which only dates and times read.
pub(super) fn cell_text(
    cell: Cell<'_>,
    kind: ColumnType,
    max: impl FnOnce() -> Option<usize>,
    out: &mut Vec<u8>,
) -> Result<bool, String> {
    match cell {
        Cell::Null => return Ok(false),
        Cell::Binary(bytes) => out.extend_from_slice(bytes),
        Cell::Int64(value) => {
            let _ = write!(out, "{value}");
        }
        Cell::Decimal128(value) => {
            let scale = match kind {
                ColumnType::Decimal128 { scale, .. } => scale,
                _ => 0,
            };
            decimal_text(value, scale, out);
        }
        Cell::Date32(days) => out.extend_from_slice(&date_text(days, max())?),
        Cell::Time32(seconds) => out.extend_from_slice(&time_text(seconds, max())?),
    }
    Ok(true)
}

/// A cell's value as a message shows it.
pub(super) fn cell_display(data: &ColumnData, row: usize) -> String {
    match data.get(row) {
        Some(Cell::Binary(bytes)) => format!("{:?}", String::from_utf8_lossy(bytes)),
        _ => data.render(row).unwrap_or_default(),
    }
}

/// One element or component of a segment being built: its place and the
/// range of its text in the builder's buffer, or nothing.
#[derive(Debug, Clone)]
pub(super) struct Part {
    pub(super) element: usize,
    pub(super) component: Option<usize>,
    pub(super) text: Option<Range<usize>>,
}

/// The bytes a segment is written with.
#[derive(Debug, Clone, Copy)]
pub(super) struct Separators {
    pub(super) element: u8,
    pub(super) component: u8,
    pub(super) segment: u8,
    pub(super) line_break: bool,
}

/// Writes segment `id` from `parts` (sorted by element, then component),
/// whose texts live in `texts`.
pub(super) fn write_segment(
    id: &[u8],
    parts: &[Part],
    texts: &[u8],
    separators: Separators,
    out: &mut Vec<u8>,
) {
    let text = |part: &Part| {
        part.text
            .as_ref()
            .and_then(|range| texts.get(range.clone()))
            .unwrap_or_default()
    };
    let last = parts
        .iter()
        .filter(|part| !text(part).is_empty())
        .map(|part| part.element)
        .max()
        .unwrap_or(0);
    out.extend_from_slice(id);
    let mut rest = parts;
    for position in 1..=last {
        out.push(separators.element);
        let count = rest
            .iter()
            .take_while(|part| part.element == position)
            .count();
        let (here, after) = rest.split_at(count);
        rest = after;
        if here.iter().all(|part| part.component.is_none()) {
            if let Some(part) = here.first() {
                out.extend_from_slice(text(part));
            }
            continue;
        }
        let last_component = here
            .iter()
            .filter(|part| !text(part).is_empty())
            .map(|part| part.component.unwrap_or(1))
            .max()
            .unwrap_or(0);
        for component in 1..=last_component {
            if component > 1 {
                out.push(separators.component);
            }
            if let Some(part) = here
                .iter()
                .find(|part| part.component.unwrap_or(1) == component)
            {
                out.extend_from_slice(text(part));
            }
        }
    }
    out.push(separators.segment);
    if separators.line_break {
        out.push(b'\n');
    }
}
