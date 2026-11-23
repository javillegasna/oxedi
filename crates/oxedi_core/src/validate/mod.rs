//! Whether a stack of package texts loads: the answer a package author asks
//! before shipping a package.
//!
//! `mod.rs` the entry point, [`validate`], which runs the checks the loader
//! already has and decides nothing itself; `report` the typed answer
//! ([`Validation`]), what it holds, the capabilities it reports and the text it
//! prints; `tests` its tests.
//!
//! The module is for tools that check packages. The browser module and the
//! built-in specs' run-time path never name it.

mod report;
#[cfg(test)]
mod tests;

pub use report::{Capability, Invalid, Member, NoPatientData, Reasons, Text, Valid, Validation};

use crate::spec_load::{LoadedSpec, Package, SpecError, Stack, Step};
use crate::write::{PlanError, WritePlan};

/// Composes the stack that `top` builds on, with the packages of `pool` as its
/// bases, and answers whether it loads: [`Validation::Yes`] with what the
/// stack holds, or [`Validation::No`] with the error that names the package,
/// the key and the value. Never fails and never panics on a text.
pub fn validate(top: &str, pool: &[&str]) -> Validation {
    match load(top, pool) {
        Ok(valid) => Validation::Yes(Box::new(valid)),
        Err(invalid) => Validation::No(invalid),
    }
}

// Reads every text, resolves and composes the stack and compiles the result.
fn load(top: &str, pool: &[&str]) -> Result<Valid, Box<Invalid>> {
    let top = Package::from_json(top).map_err(unreadable(Text::Top))?;
    let pool = read_pool(pool)?;
    let id = top.id().clone();
    let unloadable = |error: SpecError| {
        Box::new(Invalid::Unloadable {
            top: id.clone(),
            error,
        })
    };
    let stack = Stack::resolve(&top, &pool).map_err(unloadable)?;
    let member = |step: &Step| Member {
        id: step.package.id().clone(),
        role: step.role,
    };
    let bases = stack.bases().iter().map(member).collect();
    let top = member(&stack.top_step());
    let composed = stack.compose_checked().map_err(unloadable)?;
    let loaded = LoadedSpec::from_model(composed.model).map_err(unloadable)?;
    Ok(Valid {
        bases,
        top,
        write: write_capability(&loaded),
        mask: mask_capability(&loaded),
        changes: composed.changes,
        loaded,
    })
}

// Whether the write plan builds, the plan's own refusals being the reasons.
fn write_capability(loaded: &LoadedSpec) -> Capability<PlanError> {
    match WritePlan::new(&loaded.spec) {
        Ok(_) => Capability::Can,
        Err(plan) => Capability::Cannot(plan),
    }
}

// Whether the spec flags any column as patient data.
fn mask_capability(loaded: &LoadedSpec) -> Capability<NoPatientData> {
    if loaded.spec.masks_anything() {
        Capability::Can
    } else {
        Capability::Cannot(NoPatientData)
    }
}

// Reads the pool's texts, the first that does not read being the answer.
fn read_pool(pool: &[&str]) -> Result<Vec<Package>, Box<Invalid>> {
    pool.iter()
        .enumerate()
        .map(|(index, text)| {
            let position = index + 1;
            Package::from_json(text).map_err(unreadable(Text::Pool { position }))
        })
        .collect()
}

// The answer for a text that does not read, from the error it gave.
fn unreadable(text: Text) -> impl Fn(SpecError) -> Box<Invalid> {
    move |error| Box::new(Invalid::Unreadable { text, error })
}
