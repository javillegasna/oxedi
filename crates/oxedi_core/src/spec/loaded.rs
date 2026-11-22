//! A spec as a loader keeps it: its model beside the spec compiled from it.

use std::sync::Arc;

use super::model::SpecModel;
use super::{Spec, SpecError};

/// A spec as its loader keeps it: the typed model, which is patched and
/// written back, and the spec compiled from it, which the engine runs.
#[derive(Debug, Clone)]
pub struct LoadedSpec {
    /// The model the spec compiles from.
    pub model: SpecModel,
    /// The compiled spec.
    pub spec: Arc<Spec>,
}

impl LoadedSpec {
    /// Reads a spec's JSON text into its model and compiles it.
    pub fn from_json(json: &str) -> Result<LoadedSpec, SpecError> {
        SpecModel::from_json(json).and_then(LoadedSpec::from_model)
    }

    /// Compiles `model` and keeps it beside the spec; fails as `SpecModel::compile` does.
    pub fn from_model(model: SpecModel) -> Result<LoadedSpec, SpecError> {
        let spec = Arc::new(model.compile()?);
        Ok(LoadedSpec { model, spec })
    }

    /// Applies a JSON Merge Patch (RFC 7386) to the model's canonical JSON and
    /// compiles the result: objects merge recursively, arrays and scalars are
    /// replaced, `null` deletes, and a loop's occurrences, keyed by name, are
    /// added, changed or removed one by one, as on [`Spec::merge_patch`]. Every
    /// failure (the patch text, the model read, the compile) is a
    /// [`SpecError::Patch`].
    ///
    /// ```
    /// # use oxedi_core::spec::{LoadedSpec, Spec};
    /// let loaded = LoadedSpec::from_json(Spec::BUILTIN_835_JSON)?;
    /// let patched = loaded.merge_patch(
    ///     r#"{"loops":{"1000A":{"occurrences":{"extra":{"segment":"XX","pos":11400}}}}}"#,
    /// )?;
    /// assert!(patched.model.loops["1000A"].occurrences.contains_key("extra"));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn merge_patch(&self, patch_json: &str) -> Result<LoadedSpec, SpecError> {
        self.model
            .patched(patch_json)
            .and_then(LoadedSpec::from_model)
            .map_err(|source| SpecError::Patch {
                source: Box::new(source),
            })
    }

    /// The model as canonical, pretty-printed JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        self.model.to_json()
    }
}
