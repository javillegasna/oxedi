//! JSON Merge Patch, and its application to a spec's model.

use serde_json::Value;

use super::SpecError;
use super::model::SpecModel;

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

impl SpecModel {
    /// The model with the JSON Merge Patch `patch_json` applied to its
    /// canonical JSON, read back through the model; it is
    /// not compiled.
    pub(in crate::spec) fn patched(&self, patch_json: &str) -> Result<SpecModel, SpecError> {
        let patch: Value = serde_json::from_str(patch_json).map_err(SpecError::Json)?;
        let mut value = serde_json::to_value(self).map_err(SpecError::Json)?;
        merge_patch(&mut value, &patch);
        SpecModel::from_value(&value)
    }
}
