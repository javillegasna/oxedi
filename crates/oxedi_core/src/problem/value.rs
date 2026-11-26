impl fmt::Display for ConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooWide { value } => write!(f, "{value} does not fit a 64-bit integer"),
            Self::TooWideUnsigned { value } => write!(f, "{value} does not fit a 64-bit integer"),
            Self::NotWhole { value } => write!(f, "{value} is not a whole number"),
            Self::ScaleOverflow { value, from, scale } => {
                write!(f, "{value} at scale {from} overflows scale {scale}")
            }
            Self::TooManyDecimals { value, from, scale } => write!(
                f,
                "{value} at scale {from} has more decimals than the column's scale {scale}"
            ),
            Self::NotMidnight { value } => {
                write!(f, "timestamp {value} is not at midnight; give dates")
            }
            Self::NotWholeDay { millis } => write!(f, "{millis} ms is not a whole day"),
            Self::DayOutOfRange { millis } => write!(f, "{millis} ms is out of range"),
            Self::OutOfRange { value } => write!(f, "{value} is out of range"),
            Self::NotWholeSecond { value } => write!(f, "{value} is not a whole second"),
        }
    }
}

impl fmt::Display for TypeCodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeCodeError::UnknownType { found } => write!(
                f,
                "type {found:?} is not one of AN, ID, N0 to N9, R, DT, TM"
            ),
            TypeCodeError::ScaleWithoutR { kind } => {
                write!(f, "\"scale\" applies only to type R; found type {kind:?}")
            }
        }
    }
}

impl fmt::Display for ElementTextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ElementTextError::YearOutOfRange => {
                write!(f, "a date needs a year from 1 to 9999")
            }
            ElementTextError::TwoDigitYear { year } => write!(
                f,
                "the element holds a two-digit year, read as 1950 to 2049; the year is {year}"
            ),
            ElementTextError::TimeOutsideDay => write!(f, "a time must fall within one day"),
            ElementTextError::SecondsNotHeld => {
                write!(f, "the element holds hours and minutes only")
            }
            ElementTextError::NoDecimalScale { kind } => write!(
                f,
                "a decimal needs the scale of a decimal128 column; the column is {kind}"
            ),
        }
    }
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
