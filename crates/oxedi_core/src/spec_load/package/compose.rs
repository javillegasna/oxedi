//! Composing a stack: the packages' specs merged in layer order into one value.

use serde_json::{Value, json};

use super::header::Package;
use super::stack::Stack;
use crate::spec_load::error::SpecError;
use crate::spec_load::merge_patch;
use crate::spec_load::model::SpecModel;

/// A composed stack: the model read from the cumulative value.
#[derive(Debug, Clone)]
pub(in crate::spec_load) struct Composed {
    /// The model read from the value.
    pub(in crate::spec_load) model: SpecModel,
}

impl Stack<'_> {
    /// Merges each package's spec into one value in stack order and reads the
    /// whole value strictly after each package, so a refusal names the package
    /// that caused it. The name comes from the top package's id; compiling the
    /// model is left to the caller, once.
    pub(in crate::spec_load) fn compose(&self) -> Result<Composed, SpecError> {
        let mut value = json!({ "name": self.top().id().spec_name() });
        for base in self.bases() {
            absorb(&mut value, base)?;
        }
        let model = absorb(&mut value, self.top())?;
        Ok(Composed { model })
    }
}

// Merges one package into the cumulative value and reads the result.
fn absorb(value: &mut Value, package: &Package) -> Result<SpecModel, SpecError> {
    merge_patch(value, package.body());
    SpecModel::from_value(value).map_err(|source| SpecError::Package {
        id: Box::new(package.id().clone()),
        source: Box::new(source),
    })
}
