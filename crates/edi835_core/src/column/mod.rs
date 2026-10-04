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

    /// The header line: every column as `name: type`, separated by ` | `.
    pub fn header(&self) -> String {
        self.columns
            .iter()
            .map(|(name, column)| format!("{name}: {}", column.kind()))
            .collect::<Vec<_>>()
            .join(" | ")
    }

    /// Row `row` as one line, each cell rendered by [`ColumnData::render`]
    /// and separated by ` | `; `None` past the end.
    pub fn render_row(&self, row: usize) -> Option<String> {
        if row >= self.rows {
            return None;
        }
        Some(
            self.columns
                .iter()
                .map(|(_, column)| column.render(row).unwrap_or_default())
                .collect::<Vec<_>>()
                .join(" | "),
        )
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

/// The table as text: `## <name> (rows: <n>)`, the [`Table::header`], one
/// [`Table::render_row`] line per row, then an empty line.
impl fmt::Display for Table {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "## {} (rows: {})", self.name, self.rows)?;
        writeln!(f, "{}", self.header())?;
        for row in 0..self.rows {
            writeln!(f, "{}", self.render_row(row).unwrap_or_default())?;
        }
        writeln!(f)
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

/// Every table as [`Table`] displays it, in order.
impl fmt::Display for Tables {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.tables
            .iter()
            .try_for_each(|table| write!(f, "{table}"))
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
mod tests;
