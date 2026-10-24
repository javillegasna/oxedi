//! DuckDB values to the spec's column types, by the core's rules
//! (`oxedi_core::value`): integers (and whole floats) to integers, decimals
//! rescaled exactly, dates from dates or midnight timestamps, times from
//! whole seconds. A value that would change on the way is refused with the
//! reason.

use oxedi_core::value;

use super::input::Unit;

/// Ticks in a day of a timestamp counted in `unit`.
fn ticks_per_day(unit: Unit) -> i64 {
    const DAY_SECONDS: i64 = 86_400;
    match unit {
        Unit::Second => DAY_SECONDS,
        Unit::Milli => DAY_SECONDS * 1_000,
        Unit::Micro => DAY_SECONDS * 1_000_000,
        Unit::Nano => DAY_SECONDS * 1_000_000_000,
    }
}

/// An integer as `i64`, or why it does not fit.
pub fn wide_integer(value: i128) -> Result<i64, String> {
    value::integer_from_i128(value).map_err(|error| error.to_string())
}

/// An unsigned 128-bit integer as `i64`, or why it does not fit.
pub fn unsigned_integer(value: u128) -> Result<i64, String> {
    value::integer_from_u128(value).map_err(|error| error.to_string())
}

/// A float with no fraction as `i64`, or why it is refused.
pub fn float_integer(value: f64) -> Result<i64, String> {
    value::integer_from_f64(value).map_err(|error| error.to_string())
}

/// `value` at scale `from` rescaled to `scale`, or why it does not rescale
/// exactly.
pub fn rescale(value: i128, from: u8, scale: u8) -> Result<i128, String> {
    value::rescale(value, i32::from(from), scale).map_err(|error| error.to_string())
}

/// Days since 1970-01-01 of a timestamp at exactly midnight.
pub fn timestamp_date(value: i64, unit: Unit) -> Result<i32, String> {
    value::date_from_midnight(value, ticks_per_day(unit)).map_err(|error| error.to_string())
}

/// Seconds since midnight of a time counted `per_second` to the second.
pub fn whole_seconds(value: i64, per_second: i64) -> Result<i32, String> {
    value::seconds_from(value, per_second).map_err(|error| error.to_string())
}
