//! The caller's envelope values that need a check: the group's sender and
//! receiver codes, and a date or a time given in parts or in ticks.

use std::error::Error;

use crate::value::{TimeUnit, days_from_civil, seconds_of_day};

use super::envelope::Envelope;

/// The functional group's sender or receiver code.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum GroupCode {
    /// The interchange's sender (or receiver) id, trailing spaces trimmed.
    #[default]
    InterchangeId,
    /// A code the caller gives, written as it is.
    Given(String),
}

impl GroupCode {
    /// The text the group writes: the given code, or `id` without its trailing
    /// spaces.
    pub(super) fn text<'a>(&'a self, id: &'a str) -> &'a str {
        match self {
            GroupCode::InterchangeId => id.trim_end(),
            GroupCode::Given(code) => code,
        }
    }
}

/// Why a date or a time cannot be the envelope's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvelopeValueError {
    /// The year, month and day are not a calendar date.
    NotADate { year: i32, month: i32, day: i32 },
    /// A time with a fraction of a second.
    FractionalTime {
        hour: i64,
        minute: i64,
        second: i64,
        /// The fraction in billionths: `500_000_000` is half a second.
        nanosecond: u32,
    },
    /// A time outside 00:00:00 to 23:59:59.
    TimeOutOfRange { hour: i64, minute: i64, second: i64 },
    /// A time counted in ticks that is before midnight.
    TicksBeforeMidnight { ticks: i64, unit: TimeUnit },
}

impl Error for EnvelopeValueError {}

impl Envelope {
    /// Days since 1970-01-01 of a calendar date.
    pub fn date_of(year: i32, month: i32, day: i32) -> Result<i32, EnvelopeValueError> {
        days_from_civil(year, month, day).ok_or(EnvelopeValueError::NotADate { year, month, day })
    }

    /// Seconds since midnight of a time of day; a fraction of a second is
    /// refused before the range is.
    pub fn time_of(
        hour: i64,
        minute: i64,
        second: i64,
        nanosecond: u32,
    ) -> Result<i32, EnvelopeValueError> {
        if nanosecond >= 1_000_000_000 {
            return Err(EnvelopeValueError::TimeOutOfRange {
                hour,
                minute,
                second,
            });
        }
        if nanosecond != 0 {
            return Err(EnvelopeValueError::FractionalTime {
                hour,
                minute,
                second,
                nanosecond,
            });
        }
        let part = |value: i64| i32::try_from(value).ok();
        part(hour)
            .zip(part(minute))
            .zip(part(second))
            .and_then(|((hour, minute), second)| seconds_of_day(hour, minute, second))
            .ok_or(EnvelopeValueError::TimeOutOfRange {
                hour,
                minute,
                second,
            })
    }

    /// Seconds since midnight of a time counted in `unit` since midnight.
    pub fn time_from_ticks(ticks: i64, unit: TimeUnit) -> Result<i32, EnvelopeValueError> {
        let before_midnight = EnvelopeValueError::TicksBeforeMidnight { ticks, unit };
        if ticks < 0 {
            return Err(before_midnight);
        }
        let per_second = unit.ticks_per_second();
        let seconds = ticks / per_second;
        // The remainder is under one second, so its nanoseconds fit a `u32`.
        let nanosecond = u32::try_from(ticks % per_second * (1_000_000_000 / per_second))
            .map_err(|_| before_midnight)?;
        Envelope::time_of(seconds / 3600, seconds / 60 % 60, seconds % 60, nanosecond)
    }
}
