//! JSON Merge Patch.

use serde_json::Value;

/// JSON Merge Patch, RFC 7386: `patch` objects merge into `target` key by key,
/// `null` members delete, and anything that is not an object replaces `target`.
pub fn merge_patch(target: &mut Value, patch: &Value) {
    let Value::Object(members) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = Value::Object(serde_json::Map::new());
    }
    if let Value::Object(target) = target {
        for (key, value) in members {
            if value.is_null() {
                target.remove(key);
            } else {
                merge_patch(target.entry(key.as_str()).or_insert(Value::Null), value);
            }
        }
    }
}
