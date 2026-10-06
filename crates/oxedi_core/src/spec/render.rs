//! Rendering of triggers, table chains, keys and values for error messages.

use std::borrow::Cow;

use serde_json::Value;

use super::loops::Trigger;
use super::occurrences::OccurrenceDef;

/// Table names outermost first, joined by `/`; `no table` when there are none.
pub(super) fn render_chain(chain: &[String]) -> String {
    if chain.is_empty() {
        return "no table".to_string();
    }
    let names: Vec<Cow<'_, str>> = chain.iter().map(|name| render_key(name)).collect();
    names.join("/")
}

/// A trigger as `"N1" where {1: "PR", 2: "X"}`, or `"N1" with no conditions`.
pub(crate) fn render_trigger(trigger: &Trigger) -> String {
    let segment = String::from_utf8_lossy(&trigger.segment);
    if trigger.conditions.is_empty() {
        return format!("{segment:?} with no conditions");
    }
    let parts: Vec<String> = trigger
        .conditions
        .iter()
        .map(|(position, value)| format!("{position}: {:?}", String::from_utf8_lossy(value)))
        .collect();
    format!("{segment:?} where {{{}}}", parts.join(", "))
}

/// The longest qualifier code list a selector spells out.
const MAX_SELECTOR_CODES: usize = 5;

/// What selects an occurrence: `"N3"`, `"PER" where PER01 is "BL"`,
/// `"DTM" where DTM01 is one of "232", "233"`, or for a longer list
/// `"REF" where REF01 is one of 14 codes`.
pub(crate) fn render_selector(occurrence: &OccurrenceDef) -> String {
    let segment = String::from_utf8_lossy(&occurrence.segment);
    let Some(qualifier) = &occurrence.qualifier else {
        return format!("{segment:?}");
    };
    let mut at = format!("{segment}{:02}", qualifier.element);
    if let Some(component) = qualifier.component {
        at.push_str(&format!("-{component}"));
    }
    let codes = match qualifier.codes.as_slice() {
        [code] => format!("is {code:?}"),
        codes if codes.len() <= MAX_SELECTOR_CODES => {
            let quoted: Vec<String> = codes.iter().map(|code| format!("{code:?}")).collect();
            format!("is one of {}", quoted.join(", "))
        }
        codes => format!("is one of {} codes", codes.len()),
    };
    format!("{segment:?} where {at} {codes}")
}

/// The value as the datum of a [`SpecError::WrongType`]: a scalar as compact
/// JSON, a container as its length, `null` as nothing.
pub(super) fn render_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Array(items) => format!("{} items", items.len()),
        Value::Object(members) => format!("{} members", members.len()),
        scalar => scalar.to_string(),
    }
}

/// A key as it appears in a rendered path: unchanged, or JSON-quoted when it
/// is empty or holds a path separator (`.`, `/`, `#`) or whitespace, which
/// would make the path ambiguous.
pub(crate) fn render_key(key: &str) -> Cow<'_, str> {
    let ambiguous = key.is_empty()
        || key
            .chars()
            .any(|c| matches!(c, '.' | '/' | '#') || c.is_whitespace());
    if !ambiguous {
        return Cow::Borrowed(key);
    }
    Cow::Owned(serde_json::to_string(key).unwrap_or_else(|_| format!("{key:?}")))
}

/// Joins a key to the path it sits under.
pub(super) fn child(at: &str, key: &str) -> String {
    let key = render_key(key);
    if at.is_empty() {
        key.into_owned()
    } else {
        format!("{at}.{key}")
    }
}
