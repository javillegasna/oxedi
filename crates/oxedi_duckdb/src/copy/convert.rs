//! DuckDB values to the spec's column types, with the rules of the Python
//! binding's import: integers (and whole floats) to integers, decimals
//! rescaled exactly, dates from dates or midnight timestamps, times from
//! whole seconds. A value that would change on the way is refused with the
//! reason.

use super::input::Unit;

/// Milliseconds in a day.
const DAY_MILLIS: i64 = 86_400_000;

/// An integer as `i64`, or why it does not fit.
pub fn wide_integer(value: i128) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{value} does not fit a 64-bit integer"))
}

/// An unsigned 128-bit integer as `i64`, or why it does not fit.
pub fn unsigned_integer(value: u128) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{value} does not fit a 64-bit integer"))
}

/// A float with no fraction as `i64`, or why it is refused.
pub fn float_integer(value: f64) -> Result<i64, String> {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 9.2e18 {
        Ok(value as i64)
    } else {
        Err(format!("{value} is not a whole number"))
    }
}

/// `value` at scale `from` rescaled to `scale`, or why it does not rescale
/// exactly.
pub fn rescale(value: i128, from: u8, scale: u8) -> Result<i128, String> {
    let from = i32::from(from);
    let shift = i32::from(scale) - from;
    match 10i128.checked_pow(shift.unsigned_abs()) {
        Some(factor) if shift >= 0 => value
            .checked_mul(factor)
            .ok_or_else(|| format!("{value} at scale {from} overflows scale {scale}")),
        Some(factor) if value % factor == 0 => Ok(value / factor),
        _ => Err(format!(
            "{value} at scale {from} has more decimals than the column's scale {scale}"
        )),
    }
}

/// Days since 1970-01-01 of a timestamp at exactly midnight.
pub fn timestamp_date(value: i64, unit: Unit) -> Result<i32, String> {
    let per_day = match unit {
        Unit::Second => 86_400,
        Unit::Milli => DAY_MILLIS,
        Unit::Micro => DAY_MILLIS * 1_000,
        Unit::Nano => DAY_MILLIS * 1_000_000,
    };
    if value % per_day == 0 {
        i32::try_from(value / per_day).map_err(|_| format!("{value} is out of range"))
    } else {
        Err(format!("timestamp {value} is not at midnight; give dates"))
    }
}

/// Seconds since midnight of a time counted `per_second` to the second.
pub fn whole_seconds(value: i64, per_second: i64) -> Result<i32, String> {
    if value % per_second == 0 {
        i32::try_from(value / per_second).map_err(|_| format!("{value} is out of range"))
    } else {
        Err(format!("{value} is not a whole second"))
    }
}
