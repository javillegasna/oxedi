//! Checks of the JSON shape of a spec before its entries are deserialized.

use std::collections::BTreeMap;

use serde_json::Value;

use super::error::SpecError;
use super::render::{child, render_value};

/// The members of the object at `key`, in key order; empty when there is none.
pub(super) fn section<'v>(value: &'v Value, key: &str) -> BTreeMap<&'v str, &'v Value> {
    value
        .get(key)
        .and_then(Value::as_object)
        .map(|members| members.iter().map(|(k, v)| (k.as_str(), v)).collect())
        .unwrap_or_default()
}

/// How a JSON value is described when it is not the object a spec expects.
pub(super) fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// The value as an object, or [`SpecError::NotAnObject`] naming `path`.
fn object_at<'v>(
    value: &'v Value,
    path: &str,
) -> Result<&'v serde_json::Map<String, Value>, SpecError> {
    value.as_object().ok_or_else(|| SpecError::NotAnObject {
        path: path.to_string(),
        found: kind_of(value),
    })
}

/// The scalar kinds the raw structs expect.
#[derive(Clone, Copy)]
enum Leaf {
    Text,
    Flag,
    Count,
    Byte,
}

impl Leaf {
    fn expected(self) -> &'static str {
        match self {
            Leaf::Text => "a string",
            Leaf::Flag => "a boolean",
            Leaf::Count => "a non-negative integer",
            Leaf::Byte => "an integer from 0 to 255",
        }
    }

    /// What `value` is, in words, when it is not this kind; `None` when it is.
    fn mismatch(self, value: &Value) -> Option<&'static str> {
        match (self, value) {
            (Leaf::Text, Value::String(_)) | (Leaf::Flag, Value::Bool(_)) => None,
            (Leaf::Count | Leaf::Byte, Value::Number(number)) => {
                if let Some(n) = number.as_u64() {
                    if matches!(self, Leaf::Byte) && n > 255 {
                        Some("a number above 255")
                    } else {
                        None
                    }
                } else if number.is_f64() {
                    if number.as_f64().is_some_and(|float| float.fract() != 0.0) {
                        Some("a number with a fractional part")
                    } else {
                        Some("a floating-point number")
                    }
                } else {
                    Some("a negative number")
                }
            }
            _ => Some(kind_of(value)),
        }
    }
}

/// Requires `value` to be of the `kind`.
fn check_leaf(value: &Value, at: &str, kind: Leaf) -> Result<(), SpecError> {
    match kind.mismatch(value) {
        None => Ok(()),
        Some(found) => Err(SpecError::WrongType {
            path: at.to_string(),
            expected: kind.expected(),
            found,
            value: render_value(value),
        }),
    }
}

/// Requires the member `key` of `map`, when present, to be of the `kind`;
/// `null` also passes when `nullable`.
fn check_member(
    map: &serde_json::Map<String, Value>,
    at: &str,
    key: &str,
    kind: Leaf,
    nullable: bool,
) -> Result<(), SpecError> {
    match map.get(key) {
        None => Ok(()),
        Some(Value::Null) if nullable => Ok(()),
        Some(value) => check_leaf(value, &child(at, key), kind),
    }
}

/// Requires the member `key` of `map`, when present, to be an object whose
/// values are all of the `kind`.
fn check_member_map(
    map: &serde_json::Map<String, Value>,
    at: &str,
    key: &str,
    kind: Leaf,
) -> Result<(), SpecError> {
    if let Some(members) = map.get(key) {
        let at = child(at, key);
        for (name, value) in object_at(members, &at)? {
            check_leaf(value, &child(&at, name), kind)?;
        }
    }
    Ok(())
}

/// Requires the member `key` of `map`, when present, to be an array of strings.
fn check_member_texts(
    map: &serde_json::Map<String, Value>,
    at: &str,
    key: &str,
) -> Result<(), SpecError> {
    let Some(list) = map.get(key) else {
        return Ok(());
    };
    let at = child(at, key);
    let Value::Array(items) = list else {
        return Err(SpecError::WrongType {
            path: at,
            expected: "an array of strings",
            found: kind_of(list),
            value: render_value(list),
        });
    };
    for (i, item) in items.iter().enumerate() {
        check_leaf(item, &format!("{at}[{i}]"), Leaf::Text)?;
    }
    Ok(())
}

/// Requires every key of `map` to be one of `known`, then every key of
/// `required` to be present; the first unknown key in key order fails first.
fn check_keys(
    map: &serde_json::Map<String, Value>,
    at: &str,
    known: &[&str],
    required: &[&'static str],
) -> Result<(), SpecError> {
    if let Some(key) = map.keys().find(|key| !known.contains(&key.as_str())) {
        return Err(SpecError::UnknownKey {
            path: at.to_string(),
            key: key.clone(),
        });
    }
    match required.iter().find(|key| !map.contains_key(**key)) {
        Some(key) => Err(SpecError::MissingKey {
            path: at.to_string(),
            key,
        }),
        None => Ok(()),
    }
}

/// Requires an object everywhere the schema has one, only the keys the schema
/// defines and every key it requires, and the scalar kind everywhere the
/// schema has a scalar, before serde sees the value: serde would accept an
/// array in place of a struct, and its messages name neither the key path
/// nor, for objects, that an object was expected.
pub(super) fn check_shape(source: &Value) -> Result<(), SpecError> {
    let root = object_at(source, "")?;
    check_keys(
        root,
        "",
        &["name", "loops", "segments", "tables"],
        &["name", "loops"],
    )?;
    check_member(root, "", "name", Leaf::Text, false)?;
    if let Some(loops) = root.get("loops") {
        for (name, def) in object_at(loops, "loops")? {
            let at = child("loops", name);
            let def = object_at(def, &at)?;
            check_keys(
                def,
                &at,
                &["parent", "trigger", "segments", "end", "control"],
                &["trigger"],
            )?;
            check_member(def, &at, "parent", Leaf::Text, true)?;
            check_member(def, &at, "end", Leaf::Text, true)?;
            check_member_texts(def, &at, "segments")?;
            if let Some(trigger) = def.get("trigger") {
                let at = child(&at, "trigger");
                let trigger = object_at(trigger, &at)?;
                check_keys(trigger, &at, &["segment", "where"], &["segment"])?;
                check_member(trigger, &at, "segment", Leaf::Text, false)?;
                check_member_map(trigger, &at, "where", Leaf::Text)?;
            }
            if let Some(control) = def.get("control") {
                let at = child(&at, "control");
                let control = object_at(control, &at)?;
                let keys = ["opener_element", "closer_element", "count_element", "count"];
                check_keys(control, &at, &keys, &keys)?;
                for key in ["opener_element", "closer_element", "count_element"] {
                    check_member(control, &at, key, Leaf::Count, false)?;
                }
                check_member(control, &at, "count", Leaf::Text, false)?;
            }
        }
    }
    if let Some(tables) = root.get("tables") {
        for (name, def) in object_at(tables, "tables")? {
            let at = child("tables", name);
            let def = object_at(def, &at)?;
            check_keys(
                def,
                &at,
                &["loops", "ref", "segment", "repeat", "columns"],
                &["loops"],
            )?;
            check_member_texts(def, &at, "loops")?;
            check_member(def, &at, "ref", Leaf::Text, true)?;
            check_member(def, &at, "segment", Leaf::Text, true)?;
            if let Some(repeat) = def.get("repeat") {
                let at = child(&at, "repeat");
                let repeat = object_at(repeat, &at)?;
                check_keys(repeat, &at, &["from", "step"], &["from", "step"])?;
                check_member(repeat, &at, "from", Leaf::Count, false)?;
                check_member(repeat, &at, "step", Leaf::Count, false)?;
            }
            if let Some(columns) = def.get("columns") {
                let at = child(&at, "columns");
                for (column, def) in object_at(columns, &at)? {
                    let at = child(&at, column);
                    let def = object_at(def, &at)?;
                    check_keys(
                        def,
                        &at,
                        &[
                            "loop",
                            "segment",
                            "where",
                            "element",
                            "component",
                            "group_element",
                            "segment_index",
                        ],
                        &[],
                    )?;
                    check_member(def, &at, "loop", Leaf::Text, true)?;
                    check_member(def, &at, "segment", Leaf::Text, true)?;
                    check_member_map(def, &at, "where", Leaf::Text)?;
                    for key in ["element", "component", "group_element"] {
                        check_member(def, &at, key, Leaf::Count, true)?;
                    }
                    check_member(def, &at, "segment_index", Leaf::Flag, false)?;
                }
            }
        }
    }
    if let Some(segments) = root.get("segments") {
        for (id, def) in object_at(segments, "segments")? {
            let at = child("segments", id);
            let def = object_at(def, &at)?;
            check_keys(def, &at, &["elements"], &[])?;
            if let Some(elements) = def.get("elements") {
                check_elements_shape(elements, &child(&at, "elements"))?;
            }
        }
    }
    Ok(())
}

/// Requires every element (and every component) definition to be an object
/// whose scalar members are of the kind the schema expects.
fn check_elements_shape(elements: &Value, at: &str) -> Result<(), SpecError> {
    for (position, def) in object_at(elements, at)? {
        let at = child(at, position);
        let def = object_at(def, &at)?;
        check_keys(
            def,
            &at,
            &[
                "name",
                "type",
                "required",
                "min",
                "max",
                "scale",
                "composite",
            ],
            &["name", "type"],
        )?;
        check_member(def, &at, "name", Leaf::Text, false)?;
        check_member(def, &at, "type", Leaf::Text, false)?;
        check_member(def, &at, "required", Leaf::Flag, false)?;
        check_member(def, &at, "min", Leaf::Count, true)?;
        check_member(def, &at, "max", Leaf::Count, true)?;
        check_member(def, &at, "scale", Leaf::Byte, true)?;
        if let Some(composite) = def.get("composite") {
            check_elements_shape(composite, &child(&at, "composite"))?;
        }
    }
    Ok(())
}
