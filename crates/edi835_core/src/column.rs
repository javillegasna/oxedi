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

use std::fmt;

use crate::diagnostic::Quoted;
use crate::spec::ElementType;

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

/// Why a row could not be appended to a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowError {
    /// The row has a different number of cells than the table has columns.
    Arity {
        /// The table.
        table: String,
        /// Columns in the table.
        expected: usize,
        /// Cells in the row.
        found: usize,
    },
    /// One cell does not fit its column.
    Cell {
        /// The table.
        table: String,
        /// The column.
        column: String,
        /// Why the cell does not fit.
        source: CellError,
    },
}

impl fmt::Display for RowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RowError::Arity {
                table,
                expected,
                found,
            } => write!(
                f,
                "table {table:?} has {expected} columns; the row has {found} cells"
            ),
            RowError::Cell {
                table,
                column,
                source,
            } => write!(f, "table {table:?} column {column:?}: {source}"),
        }
    }
}

impl std::error::Error for RowError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RowError::Arity { .. } => None,
            RowError::Cell { source, .. } => Some(source),
        }
    }
}

/// Named columns of equal length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    name: String,
    columns: Vec<(String, ColumnData)>,
    rows: usize,
}

impl Table {
    /// An empty table with the given columns, in order.
    pub fn new(
        name: impl Into<String>,
        columns: impl IntoIterator<Item = (String, ColumnType)>,
    ) -> Table {
        Table {
            name: name.into(),
            columns: columns
                .into_iter()
                .map(|(name, kind)| (name, ColumnData::new(kind)))
                .collect(),
            rows: 0,
        }
    }

    /// The table's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Every column with its name, in order.
    pub fn columns(&self) -> &[(String, ColumnData)] {
        &self.columns
    }

    /// A column by name.
    pub fn column(&self, name: &str) -> Option<&ColumnData> {
        self.columns
            .iter()
            .find(|(column, _)| column == name)
            .map(|(_, data)| data)
    }

    /// Number of rows; every column has this length.
    pub fn len(&self) -> usize {
        self.rows
    }

    /// `true` when the table has no rows.
    pub fn is_empty(&self) -> bool {
        self.rows == 0
    }

    /// Appends one row, one cell per column in column order. Either every
    /// cell is appended or, on error, none is.
    pub fn push_row(&mut self, cells: &[Cell<'_>]) -> Result<(), RowError> {
        if cells.len() != self.columns.len() {
            return Err(RowError::Arity {
                table: self.name.clone(),
                expected: self.columns.len(),
                found: cells.len(),
            });
        }
        for ((column, data), &cell) in self.columns.iter().zip(cells) {
            data.check(cell).map_err(|source| RowError::Cell {
                table: self.name.clone(),
                column: column.clone(),
                source,
            })?;
        }
        for ((_, data), &cell) in self.columns.iter_mut().zip(cells) {
            data.push_checked(cell);
        }
        self.rows += 1;
        Ok(())
    }

    /// Moves the rows out into a new table and leaves this one empty, with
    /// the same columns and types.
    pub fn take_rows(&mut self) -> Table {
        let columns = self
            .columns
            .iter_mut()
            .map(|(name, data)| {
                let empty = ColumnData::new(data.kind());
                (name.clone(), std::mem::replace(data, empty))
            })
            .collect();
        Table {
            name: self.name.clone(),
            columns,
            rows: std::mem::take(&mut self.rows),
        }
    }
}

/// Tables ordered by name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tables {
    tables: Vec<Table>,
}

impl Tables {
    /// Collects tables, ordering them by name.
    pub fn new(mut tables: Vec<Table>) -> Tables {
        tables.sort_by(|a, b| a.name.cmp(&b.name));
        Tables { tables }
    }

    /// A table by name.
    pub fn get(&self, name: &str) -> Option<&Table> {
        self.tables.iter().find(|table| table.name == name)
    }

    /// Every table, ordered by name.
    pub fn iter(&self) -> std::slice::Iter<'_, Table> {
        self.tables.iter()
    }

    /// Number of tables.
    pub fn len(&self) -> usize {
        self.tables.len()
    }

    /// `true` when there are no tables.
    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }
}

impl<'t> IntoIterator for &'t Tables {
    type Item = &'t Table;
    type IntoIter = std::slice::Iter<'t, Table>;

    fn into_iter(self) -> Self::IntoIter {
        self.tables.iter()
    }
}

/// Splits an optional leading minus sign off `text`.
fn sign(text: &[u8]) -> (bool, &[u8]) {
    match text {
        [b'-', rest @ ..] => (true, rest),
        _ => (false, text),
    }
}

/// Appends ASCII digits to `negated`, a value kept negative so the most
/// negative number still fits; `None` on any other byte or on overflow.
fn accumulate(negated: i128, digits: &[u8]) -> Option<i128> {
    digits.iter().try_fold(negated, |value, &byte| {
        if !byte.is_ascii_digit() {
            return None;
        }
        value.checked_mul(10)?.checked_sub(i128::from(byte - b'0'))
    })
}

/// An `N` value: an optional `-` and at least one ASCII digit, nothing
/// else (no `+`, no spaces, no decimal point). The implied decimals are the
/// column's scale, so the integer is returned as written.
pub fn parse_n(text: &[u8]) -> Option<i64> {
    let (negative, digits) = sign(text);
    if digits.is_empty() {
        return None;
    }
    let negated = accumulate(0, digits)?;
    let value = if negative {
        negated
    } else {
        negated.checked_neg()?
    };
    i64::try_from(value).ok()
}

/// An `R` value scaled by `10^scale`: an optional `-`, digits, and at most
/// one `.` followed by no more than `scale` digits; at least one digit in
/// all. `None` for anything else, including spaces, a `+`, an exponent, or
/// a value of more than [`DECIMAL_PRECISION`] digits once scaled.
pub fn parse_r(text: &[u8], scale: u8) -> Option<i128> {
    let (negative, body) = sign(text);
    let (whole, fraction) = match body.iter().position(|&byte| byte == b'.') {
        Some(at) => {
            let (whole, rest) = body.split_at(at);
            (whole, rest.get(1..).unwrap_or_default())
        }
        None => (body, &[][..]),
    };
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    let missing = usize::from(scale).checked_sub(fraction.len())?;
    let mut negated = accumulate(accumulate(0, whole)?, fraction)?;
    for _ in 0..missing {
        negated = negated.checked_mul(10)?;
    }
    let limit = 10i128.checked_pow(u32::from(DECIMAL_PRECISION))?;
    if negated <= -limit {
        return None;
    }
    if negative {
        Some(negated)
    } else {
        negated.checked_neg()
    }
}

/// Up to four ASCII digits as a number.
fn small_number(digits: &[u8]) -> Option<i32> {
    if digits.is_empty() || digits.len() > 4 {
        return None;
    }
    digits.iter().try_fold(0i32, |value, &byte| {
        byte.is_ascii_digit()
            .then(|| value * 10 + i32::from(byte - b'0'))
    })
}

fn is_leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days from 1970-01-01 to a proleptic Gregorian date (H. Hinnant's
/// `days_from_civil`).
fn days_from_civil(year: i32, month: i32, day: i32) -> i32 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let march_based_month = (month + 9) % 12;
    let day_of_year = (153 * march_based_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The proleptic Gregorian date `days` after 1970-01-01 (H. Hinnant's
/// `civil_from_days`), the inverse of [`days_from_civil`].
fn civil_from_days(days: i32) -> (i32, i32, i32) {
    let shifted = days.saturating_add(719_468);
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let march_based_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * march_based_month + 2) / 5 + 1;
    let month = if march_based_month < 10 {
        march_based_month + 3
    } else {
        march_based_month - 9
    };
    let year = year_of_era + era * 400 + i32::from(month <= 2);
    (year, month, day)
}

/// A `DT` value as days since 1970-01-01: `CCYYMMDD`, or `YYMMDD` with
/// years 00–49 read as 20xx and 50–99 as 19xx. The date must exist, and the
/// year `0000` is refused (so is the all-zero date some payers write for "no
/// date"): it is not a meaningful `CCYY`.
pub fn parse_dt(text: &[u8]) -> Option<i32> {
    let (year, month_day) = match text.len() {
        8 => (small_number(text.get(..4)?)?, text.get(4..)?),
        6 => {
            let year = small_number(text.get(..2)?)?;
            let century = if year < 50 { 2000 } else { 1900 };
            (century + year, text.get(2..)?)
        }
        _ => return None,
    };
    let month = small_number(month_day.get(..2)?)?;
    let day = small_number(month_day.get(2..)?)?;
    if year < 1 || !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }
    Some(days_from_civil(year, month, day))
}

/// A `TM` value as seconds since midnight: `HHMM`, `HHMMSS`, or `HHMMSS`
/// followed by one or two decimal-second digits, which are ignored.
pub fn parse_tm(text: &[u8]) -> Option<i32> {
    if !matches!(text.len(), 4 | 6 | 7 | 8) {
        return None;
    }
    let hour = small_number(text.get(..2)?)?;
    let minute = small_number(text.get(2..4)?)?;
    let second = match text.get(4..6) {
        Some(digits) => small_number(digits)?,
        None => 0,
    };
    if text.len() > 6 {
        small_number(text.get(6..)?)?;
    }
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    Some(hour * 3600 + minute * 60 + second)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// An `R` value written the way X12 writes it, with exactly `scale` decimals.
    fn format_r(value: i128, scale: u8) -> String {
        let sign = if value < 0 { "-" } else { "" };
        let digits = value.unsigned_abs().to_string();
        let scale = usize::from(scale);
        if scale == 0 {
            return format!("{sign}{digits}");
        }
        let padded = format!("{digits:0>width$}", width = scale + 1);
        let (whole, fraction) = padded.split_at(padded.len() - scale);
        format!("{sign}{whole}.{fraction}")
    }

    #[test]
    fn column_types_follow_the_element_types() {
        assert_eq!(ColumnType::of(None), ColumnType::Binary);
        assert_eq!(ColumnType::of(Some(ElementType::An)), ColumnType::Binary);
        assert_eq!(ColumnType::of(Some(ElementType::Id)), ColumnType::Binary);
        assert_eq!(
            ColumnType::of(Some(ElementType::N(2))),
            ColumnType::Int64 { scale: 2 }
        );
        assert_eq!(
            ColumnType::of(Some(ElementType::R { scale: 4 })),
            ColumnType::Decimal128 {
                precision: 38,
                scale: 4
            }
        );
        assert_eq!(ColumnType::of(Some(ElementType::Dt)), ColumnType::Date32);
        assert_eq!(ColumnType::of(Some(ElementType::Tm)), ColumnType::Time32);
    }

    #[test]
    fn column_types_display_their_arrow_names() {
        assert_eq!(ColumnType::Binary.to_string(), "binary");
        assert_eq!(ColumnType::Int64 { scale: 0 }.to_string(), "int64");
        assert_eq!(
            ColumnType::Int64 { scale: 2 }.to_string(),
            "int64 (scale 2)"
        );
        assert_eq!(
            ColumnType::Decimal128 {
                precision: 38,
                scale: 2
            }
            .to_string(),
            "decimal128(38, 2)"
        );
        assert_eq!(ColumnType::Date32.to_string(), "date32");
        assert_eq!(ColumnType::Time32.to_string(), "time32 (seconds)");
    }

    #[test]
    fn a_bitmap_packs_bits_least_significant_first() {
        let mut bitmap = Bitmap::new();
        for valid in [true, false, true, true, false, false, false, false, true] {
            bitmap.push(valid);
        }
        assert_eq!(bitmap.len(), 9);
        assert_eq!(bitmap.as_bytes(), &[0b0000_1101, 0b0000_0001]);
        assert_eq!(bitmap.get(0), Some(true));
        assert_eq!(bitmap.get(1), Some(false));
        assert_eq!(bitmap.get(8), Some(true));
        assert_eq!(bitmap.get(9), None);
        assert_eq!(bitmap.unset_count(), 5);
        assert!(Bitmap::new().is_empty());
    }

    #[test]
    fn a_binary_column_keeps_arrow_offsets_and_repeats_them_for_nulls() {
        let mut column = ColumnData::new(ColumnType::Binary);
        column.push(Cell::Binary(b"AB")).unwrap();
        column.push_null();
        column.push(Cell::Binary(b"")).unwrap();
        column.push(Cell::Binary(b"CDE")).unwrap();
        assert_eq!(
            column.column(),
            &Column::Binary {
                offsets: vec![0, 2, 2, 2, 5],
                data: b"ABCDE".to_vec()
            }
        );
        assert_eq!(column.validity().as_bytes(), &[0b1101]);
        assert_eq!(column.get(0), Some(Cell::Binary(b"AB")));
        assert_eq!(column.get(1), Some(Cell::Null));
        assert_eq!(column.get(2), Some(Cell::Binary(b"")));
        assert_eq!(column.get(3), Some(Cell::Binary(b"CDE")));
        assert_eq!(column.get(4), None);
        assert_eq!((column.len(), column.null_count()), (4, 1));
    }

    #[test]
    fn cells_render_as_text_by_column_type() {
        let rendered = |kind: ColumnType, cells: &[Cell<'_>]| -> Vec<String> {
            let mut column = ColumnData::new(kind);
            for &cell in cells {
                column.push(cell).unwrap();
            }
            (0..column.len())
                .map(|row| column.render(row).unwrap())
                .collect()
        };
        assert_eq!(
            rendered(ColumnType::Binary, &[Cell::Binary(b"HC:99213"), Cell::Null]),
            vec!["HC:99213", "∅"]
        );
        assert_eq!(
            rendered(ColumnType::Int64 { scale: 2 }, &[Cell::Int64(-42)]),
            vec!["-42"]
        );
        assert_eq!(
            rendered(
                ColumnType::Decimal128 {
                    precision: 38,
                    scale: 2
                },
                &[
                    Cell::Decimal128(12345),
                    Cell::Decimal128(-5),
                    Cell::Decimal128(0)
                ]
            ),
            vec!["123.45", "-0.05", "0.00"]
        );
        assert_eq!(
            rendered(
                ColumnType::Decimal128 {
                    precision: 38,
                    scale: 0
                },
                &[Cell::Decimal128(7)]
            ),
            vec!["7"]
        );
        assert_eq!(
            rendered(ColumnType::Date32, &[Cell::Date32(0), Cell::Date32(19_782)]),
            vec!["1970-01-01", "2024-02-29"]
        );
        assert_eq!(
            rendered(ColumnType::Time32, &[Cell::Time32(45_045)]),
            vec!["12:30:45"]
        );
        assert_eq!(ColumnData::new(ColumnType::Binary).render(0), None);
    }

    #[test]
    fn fixed_width_columns_hold_zero_under_a_null() {
        let mut column = ColumnData::new(ColumnType::Decimal128 {
            precision: 38,
            scale: 2,
        });
        column.push(Cell::Decimal128(-1250)).unwrap();
        column.push(Cell::Null).unwrap();
        assert_eq!(
            column.column(),
            &Column::Decimal128 {
                values: vec![-1250, 0],
                precision: 38,
                scale: 2
            }
        );
        assert_eq!(column.get(1), Some(Cell::Null));
    }

    #[test]
    fn dates_and_times_out_of_range_render_as_raw_numbers() {
        let mut dates = ColumnData::new(ColumnType::Date32);
        for days in [i32::MAX, i32::MAX - 719_468, i32::MAX - 719_469, 0] {
            dates.push(Cell::Date32(days)).unwrap();
        }
        assert_eq!(dates.render(0).as_deref(), Some("date32(2147483647)"));
        assert_eq!(dates.render(1).as_deref(), Some("5879610-09-09"));
        assert_eq!(dates.render(2).as_deref(), Some("5879610-09-08"));
        assert_eq!(dates.render(3).as_deref(), Some("1970-01-01"));
        let mut times = ColumnData::new(ColumnType::Time32);
        for seconds in [-1, i32::MIN, 0, 86_399, 86_400, 90_000, i32::MAX] {
            times.push(Cell::Time32(seconds)).unwrap();
        }
        assert_eq!(times.render(0).as_deref(), Some("time32(-1)"));
        assert_eq!(times.render(1).as_deref(), Some("time32(-2147483648)"));
        assert_eq!(times.render(2).as_deref(), Some("00:00:00"));
        assert_eq!(times.render(3).as_deref(), Some("23:59:59"));
        assert_eq!(times.render(4).as_deref(), Some("time32(86400)"));
        assert_eq!(times.render(5).as_deref(), Some("time32(90000)"));
        assert_eq!(times.render(6).as_deref(), Some("time32(2147483647)"));
    }

    #[test]
    fn a_refused_text_value_is_quoted_and_cut_at_32_bytes() {
        let mut column = ColumnData::new(ColumnType::Date32);
        let long = [b'x'; 40];
        assert_eq!(
            column.push(Cell::Binary(&long)).unwrap_err().to_string(),
            format!(
                "a date32 column cannot hold a binary value (\"{}\"...)",
                "x".repeat(32)
            )
        );
    }

    #[test]
    fn a_refused_text_value_escapes_invalid_bytes_and_cuts_on_a_character() {
        let mut column = ColumnData::new(ColumnType::Date32);
        assert_eq!(
            column
                .push(Cell::Binary(b"a\xE9\"b"))
                .unwrap_err()
                .to_string(),
            "a date32 column cannot hold a binary value (\"a\\xE9\\\"b\")"
        );
        let mut text = vec![b'x'; 31];
        text.extend_from_slice("\u{e9}tail".as_bytes());
        assert_eq!(
            column.push(Cell::Binary(&text)).unwrap_err().to_string(),
            format!(
                "a date32 column cannot hold a binary value (\"{}\"...)",
                "x".repeat(31)
            )
        );
        let mut exact = vec![b'x'; 30];
        exact.extend_from_slice("\u{e9}tail".as_bytes());
        assert_eq!(
            column.push(Cell::Binary(&exact)).unwrap_err().to_string(),
            format!(
                "a date32 column cannot hold a binary value (\"{}\u{e9}\"...)",
                "x".repeat(30)
            )
        );
    }

    #[test]
    fn a_refused_run_of_stray_continuation_bytes_keeps_its_cut_at_32_bytes() {
        let mut column = ColumnData::new(ColumnType::Date32);
        assert_eq!(
            column
                .push(Cell::Binary(&[0x80; 40]))
                .unwrap_err()
                .to_string(),
            format!(
                "a date32 column cannot hold a binary value (\"{}\"...)",
                "\\x80".repeat(32)
            )
        );
    }

    #[test]
    fn a_cell_of_another_type_is_refused_and_nothing_is_appended() {
        let mut column = ColumnData::new(ColumnType::Date32);
        assert_eq!(
            column.push(Cell::Int64(3)),
            Err(CellError::TypeMismatch {
                column: ColumnType::Date32,
                cell: "int64",
                value: "3".into()
            })
        );
        assert!(column.is_empty());
    }

    #[test]
    fn cell_errors_display_the_column_and_the_cell() {
        assert_eq!(
            CellError::TypeMismatch {
                column: ColumnType::Date32,
                cell: "int64",
                value: "3".into()
            }
            .to_string(),
            "a date32 column cannot hold an int64 value (3)"
        );
        assert_eq!(
            CellError::TypeMismatch {
                column: ColumnType::Int64 { scale: 0 },
                cell: "date32",
                value: "-1".into()
            }
            .to_string(),
            "an int64 column cannot hold a date32 value (-1)"
        );
        assert_eq!(
            CellError::BinaryOverflow { bytes: 2147483650 }.to_string(),
            "a binary column holds at most 2147483647 bytes; this value would bring it to 2147483650"
        );
    }

    #[test]
    fn a_row_is_appended_whole_or_not_at_all() {
        let mut table = Table::new(
            "t",
            [
                ("id".to_string(), ColumnType::Binary),
                ("amount".to_string(), ColumnType::Int64 { scale: 0 }),
            ],
        );
        table
            .push_row(&[Cell::Binary(b"A"), Cell::Int64(1)])
            .unwrap();
        let err = table
            .push_row(&[Cell::Binary(b"B"), Cell::Binary(b"x")])
            .unwrap_err();
        assert_eq!(
            err,
            RowError::Cell {
                table: "t".into(),
                column: "amount".into(),
                source: CellError::TypeMismatch {
                    column: ColumnType::Int64 { scale: 0 },
                    cell: "binary",
                    value: "\"x\"".into()
                }
            }
        );
        assert_eq!(
            table.push_row(&[Cell::Null]),
            Err(RowError::Arity {
                table: "t".into(),
                expected: 2,
                found: 1
            })
        );
        assert_eq!(table.len(), 1);
        for (_, column) in table.columns() {
            assert_eq!(column.len(), 1);
        }
        assert_eq!(table.column("id").unwrap().get(0), Some(Cell::Binary(b"A")));
        assert_eq!(table.column("missing"), None);
    }

    #[test]
    fn row_errors_display_the_table_and_the_column() {
        let arity = RowError::Arity {
            table: "claims".into(),
            expected: 5,
            found: 4,
        };
        assert_eq!(
            arity.to_string(),
            "table \"claims\" has 5 columns; the row has 4 cells"
        );
        assert!(std::error::Error::source(&arity).is_none());
        let cell = RowError::Cell {
            table: "claims".into(),
            column: "charge".into(),
            source: CellError::TypeMismatch {
                column: ColumnType::Decimal128 {
                    precision: 38,
                    scale: 2,
                },
                cell: "binary",
                value: "\"ab\"".into(),
            },
        };
        assert_eq!(
            cell.to_string(),
            "table \"claims\" column \"charge\": a decimal128(38, 2) column cannot hold a binary value (\"ab\")"
        );
        assert!(std::error::Error::source(&cell).is_some());
    }

    #[test]
    fn taking_rows_leaves_an_empty_table_with_the_same_columns() {
        let mut table = Table::new("t", [("n".to_string(), ColumnType::Int64 { scale: 2 })]);
        table.push_row(&[Cell::Int64(7)]).unwrap();
        let taken = table.take_rows();
        assert_eq!(taken.len(), 1);
        assert_eq!(taken.column("n").unwrap().get(0), Some(Cell::Int64(7)));
        assert!(table.is_empty());
        assert_eq!(
            table.column("n").unwrap().kind(),
            ColumnType::Int64 { scale: 2 }
        );
    }

    #[test]
    fn tables_are_ordered_by_name() {
        let tables = Tables::new(vec![Table::new("services", []), Table::new("claims", [])]);
        let names: Vec<&str> = tables.iter().map(Table::name).collect();
        assert_eq!(names, vec!["claims", "services"]);
        assert_eq!(tables.get("services").map(Table::name), Some("services"));
        assert_eq!(tables.len(), 2);
    }

    #[test]
    fn n_values_are_digits_with_an_optional_minus() {
        assert_eq!(parse_n(b"0"), Some(0));
        assert_eq!(parse_n(b"007"), Some(7));
        assert_eq!(parse_n(b"-5"), Some(-5));
        assert_eq!(parse_n(b"9223372036854775807"), Some(i64::MAX));
        assert_eq!(parse_n(b"-9223372036854775808"), Some(i64::MIN));
        for text in [
            &b""[..],
            b"-",
            b"+5",
            b"1.0",
            b" 1",
            b"1 ",
            b"1a",
            b"9223372036854775808",
        ] {
            assert_eq!(parse_n(text), None, "{:?}", String::from_utf8_lossy(text));
        }
    }

    #[test]
    fn r_values_are_scaled_and_never_lose_a_decimal() {
        assert_eq!(parse_r(b"12.34", 2), Some(1234));
        assert_eq!(parse_r(b"12.3", 2), Some(1230));
        assert_eq!(parse_r(b"12", 2), Some(1200));
        assert_eq!(parse_r(b"-0.5", 2), Some(-50));
        assert_eq!(parse_r(b".5", 2), Some(50));
        assert_eq!(parse_r(b"5.", 2), Some(500));
        assert_eq!(parse_r(b"12", 0), Some(12));
        assert_eq!(
            parse_r(b"99999999999999999999.999999999999999999", 18),
            Some(99_999_999_999_999_999_999_999_999_999_999_999_999)
        );
        for (text, scale) in [
            (&b"12.345"[..], 2),
            (b"12.0", 0),
            (b"", 2),
            (b".", 2),
            (b"-", 2),
            (b"1.2.3", 2),
            (b"1e5", 2),
            (b" 1", 2),
            (b"1 ", 2),
            (b"+1", 2),
            (b"1,5", 2),
            (b"100000000000000000000", 18),
        ] {
            assert_eq!(
                parse_r(text, scale),
                None,
                "{:?} at scale {scale}",
                String::from_utf8_lossy(text)
            );
        }
    }

    #[test]
    fn dt_values_are_real_calendar_dates() {
        assert_eq!(parse_dt(b"19700101"), Some(0));
        assert_eq!(parse_dt(b"700101"), Some(0));
        assert_eq!(parse_dt(b"19691231"), Some(-1));
        assert_eq!(parse_dt(b"20240229"), Some(19_782));
        assert_eq!(parse_dt(b"20000229"), Some(11_016));
        assert_eq!(parse_dt(b"491231"), parse_dt(b"20491231"));
        assert_eq!(parse_dt(b"500101"), parse_dt(b"19500101"));
        for text in [
            &b"20230229"[..],
            b"19000229",
            b"20241301",
            b"20240100",
            b"20240431",
            b"2024011",
            b"2024-01-01",
            b"",
            b"240229 ",
            b"00000101",
            b"00000000",
        ] {
            assert_eq!(parse_dt(text), None, "{:?}", String::from_utf8_lossy(text));
        }
    }

    #[test]
    fn tm_values_are_seconds_since_midnight() {
        assert_eq!(parse_tm(b"0000"), Some(0));
        assert_eq!(parse_tm(b"1230"), Some(45_000));
        assert_eq!(parse_tm(b"123045"), Some(45_045));
        assert_eq!(parse_tm(b"1230459"), Some(45_045));
        assert_eq!(parse_tm(b"12304599"), Some(45_045));
        assert_eq!(parse_tm(b"235959"), Some(86_399));
        for text in [
            &b"2400"[..],
            b"1260",
            b"123060",
            b"123",
            b"12304",
            b"123045999",
            b"12a0",
            b"",
            b"12:30",
        ] {
            assert_eq!(parse_tm(text), None, "{:?}", String::from_utf8_lossy(text));
        }
    }

    proptest! {
        #[test]
        fn valid_n_values_round_trip(value in any::<i64>()) {
            prop_assert_eq!(parse_n(value.to_string().as_bytes()), Some(value));
        }

        #[test]
        fn valid_r_values_round_trip(
            scale in 0u8..=18,
            value in -(10i128.pow(30))..10i128.pow(30),
        ) {
            prop_assert_eq!(parse_r(format_r(value, scale).as_bytes(), scale), Some(value));
        }

        #[test]
        fn valid_dates_round_trip(year in 1i32..=9999, month in 1i32..=12, day in 1i32..=31) {
            prop_assume!(day <= days_in_month(year, month));
            let text = format!("{year:04}{month:02}{day:02}");
            let days = parse_dt(text.as_bytes());
            prop_assert!(days.is_some(), "{}", text);
            prop_assert_eq!(days.map(civil_from_days), Some((year, month, day)));
        }

        #[test]
        fn valid_times_round_trip(seconds in 0i32..86_400) {
            let text = format!(
                "{:02}{:02}{:02}",
                seconds / 3600,
                seconds / 60 % 60,
                seconds % 60
            );
            prop_assert_eq!(parse_tm(text.as_bytes()), Some(seconds));
        }

        #[test]
        fn random_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..24), scale in 0u8..=40) {
            let _ = parse_n(&bytes);
            let _ = parse_r(&bytes, scale);
            let _ = parse_dt(&bytes);
            let _ = parse_tm(&bytes);
        }

        #[test]
        fn rows_read_back_with_their_validity(
            rows in proptest::collection::vec(
                (
                    proptest::option::of(proptest::collection::vec(any::<u8>(), 0..8)),
                    proptest::option::of(any::<i64>()),
                    proptest::option::of(any::<i128>()),
                    proptest::option::of(any::<i32>()),
                ),
                0..40,
            )
        ) {
            let mut table = Table::new(
                "t",
                [
                    ("b".to_string(), ColumnType::Binary),
                    ("n".to_string(), ColumnType::Int64 { scale: 0 }),
                    ("r".to_string(), ColumnType::Decimal128 { precision: 38, scale: 2 }),
                    ("d".to_string(), ColumnType::Date32),
                    ("t".to_string(), ColumnType::Time32),
                ],
            );
            for (b, n, r, d) in &rows {
                let cells = [
                    b.as_deref().map_or(Cell::Null, Cell::Binary),
                    n.map_or(Cell::Null, Cell::Int64),
                    r.map_or(Cell::Null, Cell::Decimal128),
                    d.map_or(Cell::Null, Cell::Date32),
                    d.map_or(Cell::Null, Cell::Time32),
                ];
                prop_assert!(table.push_row(&cells).is_ok());
            }
            prop_assert_eq!(table.len(), rows.len());
            for (i, (b, n, r, d)) in rows.iter().enumerate() {
                let cell = |name: &str| table.column(name).and_then(|column| column.get(i));
                prop_assert_eq!(cell("b"), Some(b.as_deref().map_or(Cell::Null, Cell::Binary)));
                prop_assert_eq!(cell("n"), Some(n.map_or(Cell::Null, Cell::Int64)));
                prop_assert_eq!(cell("r"), Some(r.map_or(Cell::Null, Cell::Decimal128)));
                prop_assert_eq!(cell("d"), Some(d.map_or(Cell::Null, Cell::Date32)));
                prop_assert_eq!(cell("t"), Some(d.map_or(Cell::Null, Cell::Time32)));
                prop_assert_eq!(
                    table.column("b").and_then(|column| column.validity().get(i)),
                    Some(b.is_some())
                );
            }
            for (_, column) in table.columns() {
                prop_assert_eq!(column.len(), rows.len());
                prop_assert_eq!(column.validity().as_bytes().len(), rows.len().div_ceil(8));
            }
        }
    }
}
