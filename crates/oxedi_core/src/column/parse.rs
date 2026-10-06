//! Parsers from X12 element text to column values, and the civil-date arithmetic they use.

use super::DECIMAL_PRECISION;

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

pub(super) fn days_in_month(year: i32, month: i32) -> i32 {
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

/// A `DT` value as days since 1970-01-01: `CCYYMMDD`, or `YYMMDD` with
/// years 00–49 read as 20xx and 50–99 as 19xx. The date must exist, and the
/// year `0000` is refused (so is the all-zero date some payers write for "no
/// date"): it is not a meaningful `CCYY`.
pub fn parse_dt(text: &[u8]) -> Option<i32> {
    let (year, month, day) = civil_date(text)?;
    Some(days_from_civil(year, month, day))
}

/// Whether [`parse_dt`] accepts `text`, without counting the days.
pub(crate) fn is_dt(text: &[u8]) -> bool {
    civil_date(text).is_some()
}

/// The year, month and day of a `DT` value, checked as [`parse_dt`] describes.
fn civil_date(text: &[u8]) -> Option<(i32, i32, i32)> {
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
    Some((year, month, day))
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
