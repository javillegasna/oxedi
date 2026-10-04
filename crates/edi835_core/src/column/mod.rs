//! Typed columns laid out the way Apache Arrow lays out its arrays.
//!
//! A [`ColumnData`] is a value buffer plus a validity [`Bitmap`] (one bit per
//! row, least significant bit first, set when the row holds a value). Value
//! buffers follow Arrow's physical layouts: `Binary` keeps `i32` offsets
//! (one more than the rows, starting at 0) into one byte buffer, and the
//! fixed-width types keep one slot per row, zero for a null row. A
//! [`Table`] is a named list of columns that always have the same length,
//! and [`Tables`] is a list of tables ordered by name.
//!
//! The parsers turn X12 element text into column values without allocating:
//! [`parse_n`], [`parse_r`], [`parse_dt`] and [`parse_tm`].
//!
//! The module is split by responsibility: `parse` holds the parsers and the
//! date arithmetic; `table` the tables and the row appends. This file holds
//! the column types, the bitmap, the cells and the column buffers.

use std::fmt;

use crate::diagnostic::Quoted;
use crate::spec::ElementType;

mod parse;
mod table;

pub(crate) use parse::is_dt;
pub use parse::{parse_dt, parse_n, parse_r, parse_tm};
pub use table::{RowError, Table, Tables};

use parse::civil_from_days;

/// Precision of every `Decimal128` column: the most digits an `i128` holds in full.
pub const DECIMAL_PRECISION: u8 = 38;

/// The physical type of a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnType {
    /// Raw bytes, as they appear in the file.
    Binary,
    /// A 64-bit integer; `scale` implied decimal places (the `n` of `Nn`).
    Int64 {
        /// Implied decimal places.
        scale: u8,
    },
    /// A 128-bit integer scaled by `10^scale`.
    Decimal128 {
        /// Total significant digits.
        precision: u8,
        /// Decimal places.
        scale: u8,
    },
    /// Days since 1970-01-01.
    Date32,
    /// Seconds since midnight.
    Time32,
}

impl ColumnType {
    /// The column type that holds values of an element type; an element
    /// with no definition is held as raw bytes.
    pub fn of(kind: Option<ElementType>) -> ColumnType {
        match kind {
            None | Some(ElementType::An | ElementType::Id) => ColumnType::Binary,
            Some(ElementType::N(scale)) => ColumnType::Int64 { scale },
            Some(ElementType::R { scale }) => ColumnType::Decimal128 {
                precision: DECIMAL_PRECISION,
                scale,
            },
            Some(ElementType::Dt) => ColumnType::Date32,
            Some(ElementType::Tm) => ColumnType::Time32,
        }
    }
}

impl fmt::Display for ColumnType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColumnType::Binary => write!(f, "binary"),
            ColumnType::Int64 { scale: 0 } => write!(f, "int64"),
            ColumnType::Int64 { scale } => write!(f, "int64 (scale {scale})"),
            ColumnType::Decimal128 { precision, scale } => {
                write!(f, "decimal128({precision}, {scale})")
            }
            ColumnType::Date32 => write!(f, "date32"),
            ColumnType::Time32 => write!(f, "time32 (seconds)"),
        }
    }
}

/// One validity bit per row, least significant bit first; a set bit means
/// the row holds a value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bitmap {
    bytes: Vec<u8>,
    len: usize,
}

impl Bitmap {
    /// An empty bitmap.
    pub fn new() -> Bitmap {
        Bitmap::default()
    }

    /// Appends one bit.
    pub fn push(&mut self, valid: bool) {
        let bit = self.len % 8;
        if bit == 0 {
            self.bytes.push(0);
        }
        if valid && let Some(last) = self.bytes.last_mut() {
            *last |= 1 << bit;
        }
        self.len += 1;
    }

    /// The bit of row `index`; `None` past the end.
    pub fn get(&self, index: usize) -> Option<bool> {
        if index >= self.len {
            return None;
        }
        self.bytes
            .get(index / 8)
            .map(|byte| (byte >> (index % 8)) & 1 == 1)
    }

    /// Number of bits.
    pub fn len(&self) -> usize {
        self.len
    }

    /// `true` when the bitmap holds no bits.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Number of unset bits.
    pub fn unset_count(&self) -> usize {
        let set: usize = self
            .bytes
            .iter()
            .map(|byte| byte.count_ones() as usize)
            .sum();
        self.len.saturating_sub(set)
    }

    /// The packed bits; bits past `len` in the last byte are zero.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// The value buffers of one column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Column {
    /// Raw bytes: row `i` is `data[offsets[i]..offsets[i + 1]]`.
    Binary {
        /// One more offset than rows; the first is 0.
        offsets: Vec<i32>,
        /// Every row's bytes, back to back.
        data: Vec<u8>,
    },
    /// 64-bit integers with implied decimals.
    Int64 {
        /// One value per row.
        values: Vec<i64>,
        /// Implied decimal places.
        scale: u8,
    },
    /// Scaled 128-bit integers.
    Decimal128 {
        /// One value per row, scaled by `10^scale`.
        values: Vec<i128>,
        /// Total significant digits.
        precision: u8,
        /// Decimal places.
        scale: u8,
    },
    /// Days since 1970-01-01, one per row.
    Date32(Vec<i32>),
    /// Seconds since midnight, one per row.
    Time32(Vec<i32>),
}

impl Column {
    fn new(kind: ColumnType) -> Column {
        match kind {
            ColumnType::Binary => Column::Binary {
                offsets: vec![0],
                data: Vec::new(),
            },
            ColumnType::Int64 { scale } => Column::Int64 {
                values: Vec::new(),
                scale,
            },
            ColumnType::Decimal128 { precision, scale } => Column::Decimal128 {
                values: Vec::new(),
                precision,
                scale,
            },
            ColumnType::Date32 => Column::Date32(Vec::new()),
            ColumnType::Time32 => Column::Time32(Vec::new()),
        }
    }
}

/// The most bytes a binary column holds.
#[cfg(not(test))]
fn offset_limit() -> usize {
    i32::MAX as usize
}

#[cfg(test)]
thread_local! {
    pub(crate) static OFFSET_LIMIT: std::cell::Cell<usize> =
        const { std::cell::Cell::new(i32::MAX as usize) };
}

/// The most bytes a binary column holds; tests lower it per thread.
#[cfg(test)]
fn offset_limit() -> usize {
    OFFSET_LIMIT.with(std::cell::Cell::get)
}

/// One value on its way into, or out of, a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell<'a> {
    /// No value.
    Null,
    /// Raw bytes.
    Binary(&'a [u8]),
    /// An integer with the column's implied decimals.
    Int64(i64),
    /// An integer scaled by the column's `10^scale`.
    Decimal128(i128),
    /// Days since 1970-01-01.
    Date32(i32),
    /// Seconds since midnight.
    Time32(i32),
}

impl Cell<'_> {
    /// The cell's value as text for an error message: numbers as stored,
    /// bytes quoted on one line with `\xNN` for invalid bytes, cut at 32
    /// bytes without splitting a character.
    fn raw_text(&self) -> String {
        const CUT: usize = 32;
        match self {
            Cell::Null => "null".to_string(),
            Cell::Binary(bytes) => {
                let mut end = CUT.min(bytes.len());
                // A continuation byte at the cut may belong to a character
                // that began up to three bytes earlier; step back to its lead
                // byte. Without a lead byte in reach the cut stays.
                if bytes.get(end).is_some_and(|byte| byte & 0xC0 == 0x80) {
                    for back in 1..=3 {
                        let Some(at) = end.checked_sub(back) else {
                            break;
                        };
                        match bytes.get(at) {
                            Some(byte) if byte & 0xC0 == 0x80 => {}
                            Some(byte) => {
                                if *byte >= 0xC0 {
                                    end = at;
                                }
                                break;
                            }
                            None => break,
                        }
                    }
                }
                let shown = bytes.get(..end).unwrap_or(bytes);
                let text = Quoted(shown).to_string();
                if shown.len() < bytes.len() {
                    format!("{text}...")
                } else {
                    text
                }
            }
            Cell::Int64(value) => value.to_string(),
            Cell::Decimal128(value) => value.to_string(),
            Cell::Date32(value) | Cell::Time32(value) => value.to_string(),
        }
    }

    fn kind_name(&self) -> &'static str {
        match self {
            Cell::Null => "null",
            Cell::Binary(_) => "binary",
            Cell::Int64(_) => "int64",
            Cell::Decimal128(_) => "decimal128",
            Cell::Date32(_) => "date32",
            Cell::Time32(_) => "time32",
        }
    }
}

/// Why a cell could not be pushed into a column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellError {
    /// The cell's type is not the column's.
    TypeMismatch {
        /// The column's type.
        column: ColumnType,
        /// The cell's type, e.g. `int64`.
        cell: &'static str,
        /// The cell's value: integers as written, a decimal as its scaled
        /// integer, dates as days and times as seconds, bytes as a quoted
        /// string (`\xNN` for invalid bytes) cut at 32 bytes on a character
        /// boundary.
        value: String,
    },
    /// The bytes would take a binary column past what `i32` offsets address.
    BinaryOverflow {
        /// The column's byte length the value would have produced.
        bytes: usize,
    },
}

impl fmt::Display for CellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CellError::TypeMismatch {
                column,
                cell,
                value,
            } => {
                write!(
                    f,
                    "{} {column} column cannot hold {} {cell} value ({value})",
                    article(&column.to_string()),
                    article(cell)
                )
            }
            CellError::BinaryOverflow { bytes } => write!(
                f,
                "a binary column holds at most {} bytes; this value would bring it to {bytes}",
                i32::MAX
            ),
        }
    }
}

/// `an` before a word that starts with a vowel, `a` otherwise.
fn article(word: &str) -> &'static str {
    if word.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    }
}

impl std::error::Error for CellError {}

/// One typed column and the validity of each of its rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnData {
    column: Column,
    validity: Bitmap,
}

impl ColumnData {
    /// An empty column of the given type.
    pub fn new(kind: ColumnType) -> ColumnData {
        ColumnData {
            column: Column::new(kind),
            validity: Bitmap::new(),
        }
    }

    /// The column's type.
    pub fn kind(&self) -> ColumnType {
        match &self.column {
            Column::Binary { .. } => ColumnType::Binary,
            Column::Int64 { scale, .. } => ColumnType::Int64 { scale: *scale },
            Column::Decimal128 {
                precision, scale, ..
            } => ColumnType::Decimal128 {
                precision: *precision,
                scale: *scale,
            },
            Column::Date32(_) => ColumnType::Date32,
            Column::Time32(_) => ColumnType::Time32,
        }
    }

    /// The value buffers.
    pub fn column(&self) -> &Column {
        &self.column
    }

    /// The validity bitmap.
    pub fn validity(&self) -> &Bitmap {
        &self.validity
    }

    /// Number of rows.
    pub fn len(&self) -> usize {
        self.validity.len()
    }

    /// `true` when the column has no rows.
    pub fn is_empty(&self) -> bool {
        self.validity.is_empty()
    }

    /// Number of null rows.
    pub fn null_count(&self) -> usize {
        self.validity.unset_count()
    }

    /// The cell of row `row`; `None` past the end.
    pub fn get(&self, row: usize) -> Option<Cell<'_>> {
        if !self.validity.get(row)? {
            return Some(Cell::Null);
        }
        Some(match &self.column {
            Column::Binary { offsets, data } => {
                let start = usize::try_from(*offsets.get(row)?).ok()?;
                let end = usize::try_from(*offsets.get(row.checked_add(1)?)?).ok()?;
                Cell::Binary(data.get(start..end)?)
            }
            Column::Int64 { values, .. } => Cell::Int64(*values.get(row)?),
            Column::Decimal128 { values, .. } => Cell::Decimal128(*values.get(row)?),
            Column::Date32(values) => Cell::Date32(*values.get(row)?),
            Column::Time32(values) => Cell::Time32(*values.get(row)?),
        })
    }

    /// Row `row` as text: bytes as UTF-8 (invalid sequences replaced),
    /// integers as digits, decimals in fixed point with the column's scale,
    /// dates as `YYYY-MM-DD`, times as `HH:MM:SS` and a null as `∅`; `None`
    /// past the end. A date too far from 1970 to convert and a time outside
    /// one day render as their raw number, `date32(2147483647)` or
    /// `time32(90000)`.
    pub fn render(&self, row: usize) -> Option<String> {
        Some(match self.get(row)? {
            Cell::Null => "∅".to_string(),
            Cell::Binary(bytes) => String::from_utf8_lossy(bytes).into_owned(),
            Cell::Int64(value) => value.to_string(),
            Cell::Decimal128(value) => {
                let scale = match self.kind() {
                    ColumnType::Decimal128 { scale, .. } => usize::from(scale),
                    _ => 0,
                };
                let sign = if value < 0 { "-" } else { "" };
                let digits = format!("{:0>width$}", value.unsigned_abs(), width = scale + 1);
                let (whole, fraction) = digits.split_at(digits.len().saturating_sub(scale));
                if fraction.is_empty() {
                    format!("{sign}{whole}")
                } else {
                    format!("{sign}{whole}.{fraction}")
                }
            }
            Cell::Date32(days) if days.checked_add(719_468).is_none() => {
                format!("date32({days})")
            }
            Cell::Date32(days) => {
                let (year, month, day) = civil_from_days(days);
                format!("{year:04}-{month:02}-{day:02}")
            }
            Cell::Time32(seconds) if !(0..86_400).contains(&seconds) => {
                format!("time32({seconds})")
            }
            Cell::Time32(seconds) => format!(
                "{:02}:{:02}:{:02}",
                seconds / 3600,
                seconds / 60 % 60,
                seconds % 60
            ),
        })
    }

    /// Appends a null row.
    pub fn push_null(&mut self) {
        match &mut self.column {
            Column::Binary { offsets, .. } => {
                let last = offsets.last().copied().unwrap_or_default();
                offsets.push(last);
            }
            Column::Int64 { values, .. } => values.push(0),
            Column::Decimal128 { values, .. } => values.push(0),
            Column::Date32(values) | Column::Time32(values) => values.push(0),
        }
        self.validity.push(false);
    }

    /// Appends one row. A [`Cell::Null`] fits any column; any other cell
    /// must match the column's type. On error nothing is appended.
    pub fn push(&mut self, cell: Cell<'_>) -> Result<(), CellError> {
        self.check(cell)?;
        self.push_checked(cell);
        Ok(())
    }

    /// Whether `cell` can be appended.
    fn check(&self, cell: Cell<'_>) -> Result<(), CellError> {
        let fits = matches!(
            (&self.column, cell),
            (_, Cell::Null)
                | (Column::Binary { .. }, Cell::Binary(_))
                | (Column::Int64 { .. }, Cell::Int64(_))
                | (Column::Decimal128 { .. }, Cell::Decimal128(_))
                | (Column::Date32(_), Cell::Date32(_))
                | (Column::Time32(_), Cell::Time32(_))
        );
        if !fits {
            return Err(CellError::TypeMismatch {
                column: self.kind(),
                cell: cell.kind_name(),
                value: cell.raw_text(),
            });
        }
        if let (Column::Binary { data, .. }, Cell::Binary(bytes)) = (&self.column, cell) {
            let total = data.len().saturating_add(bytes.len());
            if total > offset_limit() {
                return Err(CellError::BinaryOverflow { bytes: total });
            }
        }
        Ok(())
    }

    /// Appends a cell that [`ColumnData::check`] accepted.
    fn push_checked(&mut self, cell: Cell<'_>) {
        if cell == Cell::Null {
            self.push_null();
            return;
        }
        match (&mut self.column, cell) {
            (Column::Binary { offsets, data }, Cell::Binary(bytes)) => {
                data.extend_from_slice(bytes);
                offsets.push(i32::try_from(data.len()).unwrap_or(i32::MAX));
            }
            (Column::Int64 { values, .. }, Cell::Int64(value)) => values.push(value),
            (Column::Decimal128 { values, .. }, Cell::Decimal128(value)) => values.push(value),
            (Column::Date32(values), Cell::Date32(value))
            | (Column::Time32(values), Cell::Time32(value)) => values.push(value),
            _ => {
                self.push_null();
                return;
            }
        }
        self.validity.push(true);
    }
}

#[cfg(test)]
mod tests;
