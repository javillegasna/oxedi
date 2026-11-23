//! Composing a stack: the packages' specs merged in layer order into one value.

use serde_json::{Value, json};

use super::header::Package;
use super::owners::check_owners;
use super::stack::Stack;
use crate::spec_load::error::SpecError;
use crate::spec_load::merge_patch;
use crate::spec_load::model::SpecModel;

/// A composed stack: the cumulative value and the model read from it.
#[derive(Debug, Clone)]
pub(in crate::spec_load) struct Composed {
    /// The packages' specs merged in stack order, under the top package's name.
    pub(in crate::spec_load) value: Value,
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
        for step in self.bases() {
            absorb(&mut value, step.package)?;
        }
        let model = absorb(&mut value, self.top())?;
        Ok(Composed { value, model })
    }
}

// Checks the keys a package writes, merges it into the cumulative value and
// reads the result.
fn absorb(value: &mut Value, package: &Package) -> Result<SpecModel, SpecError> {
    check_owners(package).map_err(|reason| SpecError::Stack { reason })?;
    merge_patch(value, package.body());
    SpecModel::from_value(value).map_err(|source| SpecError::Package {
        id: Box::new(package.id().clone()),
        source: Box::new(source),
    })
}
