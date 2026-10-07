//! The options of `COPY … (FORMAT edi835, …)`: the envelope, with the names
//! and defaults of `oxedi.Envelope`, and the spec's version.
//!
//! DuckDB hands the options over as one STRUCT value, upper-cased names to
//! values; [`OptionValue`] is that value read into Rust, so the rules here
//! do not touch the C API.

use oxedi_core::Delimiters;
use oxedi_core::write::Envelope;

use super::error::CopyError;

/// The version used when `version` is not given.
pub const DEFAULT_VERSION: &str = "5010";

/// Every option, in the order the messages list them.
pub const OPTIONS: &[&str] = &[
    "sender_id",
    "receiver_id",
    "date",
    "time",
    "sender_qualifier",
    "receiver_qualifier",
    "usage_indicator",
    "control_number",
    "application_sender",
    "application_receiver",
    "delimiters",
    "line_break",
    "version",
];

/// The options without a default.
pub const REQUIRED: &[&str] = &["sender_id", "receiver_id", "date", "time"];

/// The fields of `delimiters`, as `oxedi.Delimiters` names them.
pub const DELIMITER_FIELDS: &[&str] = &["element", "component", "segment", "repetition", "release"];

/// File options DuckDB passes on to the format, with why each does not
/// apply.
const FILE_OPTIONS: &[(&str, &str)] = &[(
    "compression",
    "the format writes one uncompressed file; compress it afterwards",
)];

const TEXT: &str = "text (VARCHAR)";
const CONTROL_NUMBER: &str = "a whole number from 0 to 18446744073709551615";
const DATE: &str = "a DATE, or text in the form YYYY-MM-DD";
const TIME: &str = "a TIME or TIME_NS, or text in the form HH:MM, HHMM or HH:MM:SS";
const BOOLEAN: &str = "a BOOLEAN";
const DELIMITERS: &str = "a STRUCT of one-byte texts with any of the fields element, component, \
                          segment, repetition and release, such as {'element': '|'}";
const DELIMITER: &str = "a one-byte VARCHAR or BLOB";
const VERSION: &str = "text (VARCHAR) naming a version";

/// One option value as DuckDB gave it.
#[derive(Debug, Clone, PartialEq)]
pub struct OptionValue {
    /// The value.
    pub kind: Kind,
    /// The value as messages show it: its DuckDB type and its text, e.g.
    /// `INTEGER 42`.
    pub shown: String,
}

/// The values an option can hold.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// No value: the option was given bare, or as NULL.
    Null,
    /// A BOOLEAN.
    Bool(bool),
    /// A VARCHAR.
    Text(String),
    /// A BLOB.
    Bytes(Vec<u8>),
    /// Any integer type.
    Integer(i128),
    /// A DATE, as days since 1970-01-01.
    Date(i32),
    /// A TIME or TIME_NS: `value` counted `per_second` to the second since
    /// midnight (microseconds, or nanoseconds).
    Time { value: i64, per_second: i64 },
    /// A STRUCT, its fields in order.
    Struct(Vec<(String, OptionValue)>),
    /// Any other type, or several values.
    Other,
}

/// What the options of one `COPY` asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The envelope to write.
    pub envelope: Envelope,
    /// The version of the built-in spec, as given (checked by the caller
    /// against the built-ins).
    pub version: String,
}

/// A required option's value.
fn required<'a>(
    given: &'a [(String, OptionValue)],
    name: &'static str,
) -> Result<&'a OptionValue, CopyError> {
    lookup(given, name).ok_or(CopyError::MissingOption {
        name,
        required: REQUIRED,
    })
}

fn lookup<'a>(given: &'a [(String, OptionValue)], name: &str) -> Option<&'a OptionValue> {
    given
        .iter()
        .find(|(option, _)| option == name)
        .map(|(_, value)| value)
}

fn wrong(name: &'static str, value: &OptionValue, expected: &'static str) -> CopyError {
    CopyError::OptionValue {
        name,
        value: value.shown.clone(),
        expected,
    }
}

/// The text of a VARCHAR option.
fn text(name: &'static str, value: &OptionValue) -> Result<String, CopyError> {
    match &value.kind {
        Kind::Text(text) => Ok(text.clone()),
        Kind::Null => Err(CopyError::NoValue { name }),
        _ => Err(wrong(name, value, TEXT)),
    }
}

fn optional_text(
    given: &[(String, OptionValue)],
    name: &'static str,
) -> Result<Option<String>, CopyError> {
    lookup(given, name)
        .map(|value| text(name, value))
        .transpose()
}

impl Settings {
    /// Reads the options, each name lower case.
    pub fn parse(given: &[(String, OptionValue)]) -> Result<Settings, CopyError> {
        for (name, _) in given {
            if let Some((_, reason)) = FILE_OPTIONS.iter().find(|(option, _)| option == name) {
                return Err(CopyError::FileOption {
                    name: name.clone(),
                    reason,
                });
            }
            if !OPTIONS.contains(&name.as_str()) {
                return Err(CopyError::UnknownOption {
                    name: name.clone(),
                    known: OPTIONS,
                });
            }
        }
        let sender_id = text("sender_id", required(given, "sender_id")?)?;
        let receiver_id = text("receiver_id", required(given, "receiver_id")?)?;
        let date = date(required(given, "date")?)?;
        let time = time(required(given, "time")?)?;
        let sender_qualifier = optional_text(given, "sender_qualifier")?;
        let receiver_qualifier = optional_text(given, "receiver_qualifier")?;
        let mut envelope = Envelope::new(
            sender_qualifier.unwrap_or_else(|| "ZZ".to_owned()),
            sender_id,
            receiver_qualifier.unwrap_or_else(|| "ZZ".to_owned()),
            receiver_id,
            date,
            time,
        );
        if let Some(usage) = optional_text(given, "usage_indicator")? {
            envelope.usage_indicator = usage;
        }
        if let Some(value) = lookup(given, "control_number") {
            envelope.control_number = control_number(value)?;
        }
        envelope.application_sender = optional_text(given, "application_sender")?;
        envelope.application_receiver = optional_text(given, "application_receiver")?;
        if let Some(value) = lookup(given, "delimiters") {
            envelope.delimiters = delimiters(value)?;
        }
        if let Some(value) = lookup(given, "line_break") {
            envelope.line_break = match value.kind {
                // A bare boolean option means true, as DuckDB's own
                // options (HEADER) do.
                Kind::Null => true,
                Kind::Bool(flag) => flag,
                _ => return Err(wrong("line_break", value, BOOLEAN)),
            };
        }
        let version = match lookup(given, "version") {
            Some(value) => match &value.kind {
                Kind::Text(version) => version.clone(),
                Kind::Null => return Err(CopyError::NoValue { name: "version" }),
                _ => return Err(wrong("version", value, VERSION)),
            },
            None => DEFAULT_VERSION.to_owned(),
        };
        Ok(Settings { envelope, version })
    }
}

fn control_number(value: &OptionValue) -> Result<u64, CopyError> {
    match value.kind {
        Kind::Integer(number) => {
            u64::try_from(number).map_err(|_| wrong("control_number", value, CONTROL_NUMBER))
        }
        Kind::Null => Err(CopyError::NoValue {
            name: "control_number",
        }),
        _ => Err(wrong("control_number", value, CONTROL_NUMBER)),
    }
}

/// Days since 1970-01-01 of a DATE or of `YYYY-MM-DD` text.
fn date(value: &OptionValue) -> Result<i32, CopyError> {
    match &value.kind {
        // DuckDB writes the infinite dates as the extremes of `i32`.
        Kind::Date(days) if *days != i32::MAX && *days != -i32::MAX => Ok(*days),
        Kind::Text(text) => parse_date(text).ok_or_else(|| wrong("date", value, DATE)),
        Kind::Null => Err(CopyError::NoValue { name: "date" }),
        _ => Err(wrong("date", value, DATE)),
    }
}

/// Seconds since midnight of a TIME, a TIME_NS or of `HH:MM`, `HHMM` or `HH:MM:SS`
/// text.
fn time(value: &OptionValue) -> Result<i32, CopyError> {
    match &value.kind {
        Kind::Time {
            value: ticks,
            per_second,
        } => {
            if ticks % per_second != 0 {
                return Err(CopyError::FractionalTime {
                    value: value.shown.clone(),
                });
            }
            i32::try_from(ticks / per_second)
                .ok()
                .filter(|seconds| (0..86_400).contains(seconds))
                .ok_or_else(|| CopyError::TimeOutOfRange {
                    value: value.shown.clone(),
                })
        }
        Kind::Text(text) => parse_time(text).ok_or_else(|| wrong("time", value, TIME)),
        Kind::Null => Err(CopyError::NoValue { name: "time" }),
        _ => Err(wrong("time", value, TIME)),
    }
}

/// The delimiters of a STRUCT, with `oxedi.Delimiters`' defaults: `*`, `:`,
/// `~`, and no repetition or release byte.
fn delimiters(value: &OptionValue) -> Result<Delimiters, CopyError> {
    let fields = match &value.kind {
        Kind::Struct(fields) => fields,
        Kind::Null => return Err(CopyError::NoValue { name: "delimiters" }),
        _ => return Err(wrong("delimiters", value, DELIMITERS)),
    };
    let mut bytes: [Option<u8>; 5] = [Some(b'*'), Some(b':'), Some(b'~'), None, None];
    for (name, field) in fields {
        let Some(index) = DELIMITER_FIELDS.iter().position(|known| known == name) else {
            return Err(CopyError::UnknownDelimiter {
                name: name.clone(),
                known: DELIMITER_FIELDS,
            });
        };
        let given: &[u8] = match &field.kind {
            Kind::Text(text) => text.as_bytes(),
            Kind::Bytes(bytes) => bytes,
            // A NULL leaves the delimiter out, as `None` does in Python;
            // the writer refuses a missing one it needs.
            Kind::Null if index >= 3 => {
                if let Some(slot) = bytes.get_mut(index) {
                    *slot = None;
                }
                continue;
            }
            _ => {
                return Err(CopyError::OptionValue {
                    name: "delimiters",
                    value: format!("{} in the field {name:?}", field.shown),
                    expected: DELIMITER,
                });
            }
        };
        let [byte] = given else {
            return Err(CopyError::DelimiterLength {
                name: name.clone(),
                bytes: given.to_vec(),
            });
        };
        if let Some(slot) = bytes.get_mut(index) {
            *slot = Some(*byte);
        }
    }
    let [
        Some(element),
        Some(component),
        Some(segment),
        repetition,
        release,
    ] = bytes
    else {
        return Err(wrong("delimiters", value, DELIMITERS));
    };
    let mut delimiters = Delimiters::new(element, component, segment);
    delimiters.repetition = repetition;
    delimiters.release = release;
    Ok(delimiters)
}

/// A number of exactly `digits` ASCII digits.
fn number(text: &str, digits: usize) -> Option<i64> {
    (text.len() == digits && text.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
}

/// Days since 1970-01-01 of a valid `YYYY-MM-DD` date.
pub fn parse_date(text: &str) -> Option<i32> {
    let mut parts = text.split('-');
    let year = number(parts.next()?, 4)?;
    let month = number(parts.next()?, 2)?;
    let day = number(parts.next()?, 2)?;
    if parts.next().is_some() || !(1..=12).contains(&month) || year == 0 {
        return None;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let length = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1..=length).contains(&day) {
        return None;
    }
    i32::try_from(days_from_civil(year, month, day)).ok()
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's
/// `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Seconds since midnight of `HH:MM`, `HHMM` or `HH:MM:SS`.
pub fn parse_time(text: &str) -> Option<i32> {
    let (hour, minute, second) = match text.len() {
        4 => (text.get(0..2)?, text.get(2..4)?, "00"),
        5 if text.as_bytes().get(2) == Some(&b':') => (text.get(0..2)?, text.get(3..5)?, "00"),
        8 if text.as_bytes().get(2) == Some(&b':') && text.as_bytes().get(5) == Some(&b':') => {
            (text.get(0..2)?, text.get(3..5)?, text.get(6..8)?)
        }
        _ => return None,
    };
    let (hour, minute, second) = (number(hour, 2)?, number(minute, 2)?, number(second, 2)?);
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    i32::try_from(hour * 3600 + minute * 60 + second).ok()
}
