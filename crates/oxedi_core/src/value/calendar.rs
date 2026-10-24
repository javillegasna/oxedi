//! Days since 1970-01-01 and seconds of the day, with the checks a caller's
//! numbers need before they become a date or a time.

/// Seconds in a day.
const DAY_SECONDS: i64 = 86_400;

fn is_leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// Days in a month of a year; `month` is 1-based.
pub(crate) fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Whether a proleptic Gregorian date exists in the years 1..=9999: the
/// month in 1..=12 and the day in the month.
pub(crate) fn is_civil(year: i32, month: i32, day: i32) -> bool {
    (1..=9999).contains(&year)
        && (1..=12).contains(&month)
        && (1..=days_in_month(year, month)).contains(&day)
}

/// Days since 1970-01-01 of a proleptic Gregorian date; `None` when the year
/// is outside 1..=9999, the month outside 1..=12 or the day outside the month.
pub fn days_from_civil(year: i32, month: i32, day: i32) -> Option<i32> {
    is_civil(year, month, day).then(|| days_from_civil_unchecked(year, month, day))
}

/// Days from 1970-01-01 to a proleptic Gregorian date (H. Hinnant's
/// `days_from_civil`); the date must already be known to exist.
pub(crate) fn days_from_civil_unchecked(year: i32, month: i32, day: i32) -> i32 {
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
pub(crate) fn civil_from_days(days: i32) -> (i32, i32, i32) {
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

/// Seconds since midnight; `None` past 23:59:59 or for a negative part.
pub fn seconds_of_day(hour: i32, minute: i32, second: i32) -> Option<i32> {
    let in_range =
        (0..24).contains(&hour) && (0..60).contains(&minute) && (0..60).contains(&second);
    in_range.then(|| hour * 3600 + minute * 60 + second)
}

/// `seconds` when it is a time of day (0..86_400), else `None`.
pub fn time_of_day(seconds: i64) -> Option<i32> {
    (0..DAY_SECONDS)
        .contains(&seconds)
        .then(|| i32::try_from(seconds).ok())
        .flatten()
}
