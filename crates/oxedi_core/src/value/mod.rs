//! The rules that turn a caller's values into the cells of a column, and the
//! calendar they share with the readers.
//!
//! The folder holds: `convert.rs` (integers, decimals, dates and times that
//! either keep their value on the way to a column type or say why not,
//! [`ConvertError`]), `calendar.rs` (days since 1970-01-01 and seconds of the
//! day, checked) and `tests/`.

mod calendar;
mod convert;

pub(crate) use calendar::{civil_from_days, days_from_civil_unchecked, days_in_month};
pub use calendar::{days_from_civil, seconds_of_day, time_of_day};
pub use convert::{
    ConvertError, MAX_SCALE, date_from_midnight, date_from_millis, integer_from_f64,
    integer_from_i128, integer_from_u128, rescale, seconds_from,
};

#[cfg(test)]
mod tests;
