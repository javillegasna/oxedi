//! Composing a stack: the packages' specs merged in layer order into one value.

use serde_json::{Value, json};

use super::change::{Change, ChangeKind};
use super::error::StackError;
use super::header::Package;
use super::narrow::differences;
use super::owners::check_owners;
use super::stack::{Role, Stack, Step};
use crate::spec_load::error::SpecError;
use crate::spec_load::merge_patch;
use crate::spec_load::model::SpecModel;

/// A composed stack: the cumulative value and the model read from it.
#[derive(Debug, Clone)]
pub struct Composed {
    /// The packages' specs merged in stack order, under the top package's name;
    /// kept for the test that ties the built-ins' lean read to this composition.
    #[cfg_attr(not(test), expect(dead_code))]
    pub(in crate::spec_load) value: Value,
    /// The model read from the value.
    pub model: SpecModel,
    /// What the editions and patches of the stack changed, added or removed,
    /// in stack order. A package that only narrows adds to its bases and
    /// narrows them, and lists nothing here.
    pub changes: Vec<Change>,
}

impl Stack<'_> {
    /// Merges each package's spec into one value in stack order and reads the
    /// whole value strictly after each package, so a refusal names the package
    /// that caused it, then compares the model after each package with the one
    /// before: a package that only narrows may not widen what its bases allow
    /// ([`StackError::Widens`]); an edition or a patch may change anything and
    /// its changes are listed in [`Composed::changes`]. The name comes from
    /// the top package's id, unless a patch writes it; compiling the model is
    /// left to the caller, once.
    pub fn compose_checked(&self) -> Result<Composed, SpecError> {
        let mut value = json!({ "name": self.top().id().spec_name() });
        let mut changes = Vec::new();
        let mut before = None;
        for step in self.bases() {
            let next = advance(&mut value, *step, before.as_ref(), &mut changes)?;
            before = Some(next);
        }
        let top = self.top_step();
        let model = advance(&mut value, top, before.as_ref(), &mut changes)?;
        Ok(Composed {
            value,
            model,
            changes,
        })
    }
}

// Merges one package, reads the result and lists what it changed from the
// model `before` it, when there is one.
fn advance(
    value: &mut Value,
    step: Step<'_>,
    before: Option<&SpecModel>,
    changes: &mut Vec<Change>,
) -> Result<SpecModel, SpecError> {
    let after = absorb(value, step.package)?;
    if let Some(prev) = before {
        changes.extend(changes_of(step, prev, &after)?);
    }
    Ok(after)
}

// The changes a package lists: all of an edition's or a patch's, additions
// included; a package that only narrows lists none, may add, and its first
// difference that is not a narrowing is refused.
fn changes_of(
    step: Step<'_>,
    prev: &SpecModel,
    next: &SpecModel,
) -> Result<Vec<Change>, SpecError> {
    let found = differences(step.package.id(), prev, next);
    if step.role != Role::Narrows {
        return Ok(found);
    }
    match found.into_iter().find(|change| {
        !matches!(
            change.kind,
            ChangeKind::Narrowed { .. } | ChangeKind::Added { .. }
        )
    }) {
        None => Ok(Vec::new()),
        Some(change) => Err(SpecError::Stack {
            reason: Box::new(StackError::Widens {
                change: Box::new(change),
            }),
        }),
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
