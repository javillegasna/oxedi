//! The text of a date or a time the envelope cannot hold.

use std::fmt;

use crate::write::EnvelopeValueError;

impl fmt::Display for EnvelopeValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotADate { year, month, day } => {
                write!(
                    f,
                    "date {year:04}-{month:02}-{day:02} is not a calendar date"
                )
            }
            Self::FractionalTime {
                hour,
                minute,
                second,
                nanosecond,
            } => {
                let digits = format!("{nanosecond:09}");
                write!(
                    f,
                    "time {hour:02}:{minute:02}:{second:02}.{} has a fraction of a second; \
                     the envelope holds whole seconds",
                    digits.trim_end_matches('0')
                )
            }
            Self::TimeOutOfRange {
                hour,
                minute,
                second,
            } => write!(
                f,
                "time {hour:02}:{minute:02}:{second:02} is out of range; it must be from \
                 00:00:00 to 23:59:59"
            ),
            Self::TicksBeforeMidnight { ticks, unit } => write!(
                f,
                "time {ticks} {}{} after midnight is before midnight; it must be from \
                 00:00:00 to 23:59:59",
                unit.word(),
                if ticks.unsigned_abs() == 1 { "" } else { "s" }
            ),
        }
    }
}
