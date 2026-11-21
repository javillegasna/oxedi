//! Checks of the JSON shape of a spec before its entries are deserialized.
//!
//! The shape is data: every object the schema defines is an [`ObjectSchema`],
//! a table of its keys and the [`Field`] each holds (the tables are emitted in
//! `model`; the helpers that build a `Field` are in `sections`), and one walk checks a value against it. The root is such a
//! table too, one row per section of the spec, so a section is checked by
//! looking up its key.

use serde_json::Value;

use super::error::SpecError;
use super::model::ROOT;
use super::render::{child, render_value};
use crate::vocabulary::Pick;

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
pub(super) fn object_at<'v>(
    value: &'v Value,
    path: &str,
) -> Result<&'v serde_json::Map<String, Value>, SpecError> {
    value.as_object().ok_or_else(|| SpecError::NotAnObject {
        path: path.to_string(),
        found: kind_of(value),
    })
}

/// The scalar kinds the model expects.
#[derive(Clone, Copy)]
pub(super) enum Leaf {
    Text,
    Flag,
    Count,
    Pick,
}

impl Leaf {
    fn expected(self) -> &'static str {
        match self {
            Leaf::Text => "a string",
            Leaf::Flag => "a boolean",
            Leaf::Count => "a non-negative integer",
            Leaf::Pick => Pick::EXPECTED,
        }
    }

    /// What `value` is, in words, when it is not this kind; `None` when it is.
    fn mismatch(self, value: &Value) -> Option<&'static str> {
        match (self, value) {
            (Leaf::Text | Leaf::Pick, Value::String(_)) | (Leaf::Flag, Value::Bool(_)) => None,
            (Leaf::Count | Leaf::Pick, Value::Number(number)) => {
                if number.is_u64() {
                    None
                } else if number.is_i64() {
                    Some("a negative number")
                } else if number.as_f64().is_some_and(|float| float.fract() != 0.0) {
                    Some("a number with a fractional part")
                } else {
                    Some("a floating-point number")
                }
            }
            _ => Some(kind_of(value)),
        }
    }
}

/// Requires `value` to be of the `kind`.
pub(super) fn check_leaf(value: &Value, at: &str, kind: Leaf) -> Result<(), SpecError> {
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

/// What a key of an object holds.
#[derive(Clone, Copy)]
pub(super) enum Field {
    /// A scalar of the kind; `null` passes too when `nullable`.
    Leaf { kind: Leaf, nullable: bool },
    /// An array whose every item is the field; `expected` describes the
    /// array in the error when the value is not one.
    List {
        item: &'static Field,
        expected: &'static str,
    },
    /// An object whose every member, whatever its key, is the field.
    Map(&'static Field),
    /// An object of the schema.
    Object(&'static ObjectSchema),
    /// A value the walk leaves to the model read, which refuses it at its path.
    Opaque,
}

/// The keys an object may hold, in the order their values are checked, and
/// the keys it must hold.
pub(super) struct ObjectSchema {
    pub(super) keys: &'static [(&'static str, Field)],
    pub(super) required: &'static [&'static str],
}

impl Field {
    /// Requires `value`, found at `at`, to be what the field describes.
    fn check(self, value: &Value, at: &str) -> Result<(), SpecError> {
        match self {
            Field::Leaf { nullable: true, .. } if value.is_null() => Ok(()),
            Field::Leaf { kind, .. } => check_leaf(value, at, kind),
            Field::List { item, expected } => check_list(value, at, item, expected),
            Field::Map(item) => check_map(value, at, item),
            Field::Object(schema) => schema.check(value, at),
            Field::Opaque => Ok(()),
        }
    }
}

/// Requires an array whose every item is `item`.
fn check_list(
    value: &Value,
    at: &str,
    item: &Field,
    expected: &'static str,
) -> Result<(), SpecError> {
    let Value::Array(items) = value else {
        return Err(SpecError::WrongType {
            path: at.to_string(),
            expected,
            found: kind_of(value),
            value: render_value(value),
        });
    };
    items
        .iter()
        .enumerate()
        .try_for_each(|(i, value)| item.check(value, &format!("{at}[{i}]")))
}

/// Requires an object whose every member is `item`.
fn check_map(value: &Value, at: &str, item: &Field) -> Result<(), SpecError> {
    object_at(value, at)?
        .iter()
        .try_for_each(|(key, value)| item.check(value, &child(at, key)))
}

impl ObjectSchema {
    /// Requires an object with only the keys of the schema, every key it
    /// requires, and each value of the field its key holds.
    fn check(&self, value: &Value, at: &str) -> Result<(), SpecError> {
        let map = object_at(value, at)?;
        self.check_keys(map, at)?;
        self.keys
            .iter()
            .filter_map(|(key, field)| map.get(*key).map(|value| (key, field, value)))
            .try_for_each(|(key, field, value)| field.check(value, &child(at, key)))
    }

    /// Requires every key of `map` to be in the schema, then every required
    /// key to be present; the first unknown key in key order fails first.
    fn check_keys(&self, map: &serde_json::Map<String, Value>, at: &str) -> Result<(), SpecError> {
        if let Some(key) = map.keys().find(|key| !self.knows(key)) {
            return Err(SpecError::UnknownKey {
                path: at.to_string(),
                key: key.clone(),
                allowed: self.names(),
            });
        }
        match self.required.iter().find(|key| !map.contains_key(**key)) {
            Some(key) => Err(SpecError::MissingKey {
                path: at.to_string(),
                key,
            }),
            None => Ok(()),
        }
    }

    fn knows(&self, key: &str) -> bool {
        self.keys.iter().any(|(known, _)| *known == key)
    }

    /// The keys of the schema, in alphabetical order.
    fn names(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = self.keys.iter().map(|(key, _)| *key).collect();
        names.sort_unstable();
        names
    }
}

/// Requires an object everywhere the schema has one, only the keys the schema
/// defines and every key it requires, and the scalar kind everywhere the
/// schema has a scalar, before serde sees the value: serde would accept an
/// array in place of a struct, and its messages name neither the key path
/// nor, for objects, that an object was expected.
pub(super) fn check_shape(source: &Value) -> Result<(), SpecError> {
    ROOT.check(source, "")
}
