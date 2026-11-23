//! A stack of packages: a top package and everything it builds on.

use super::error::StackError;
use super::header::Package;
use super::id::PackageId;
use crate::spec_load::error::SpecError;

/// A top package and every package it builds on, bases first, each once.
#[derive(Debug, Clone)]
pub(in crate::spec_load) struct Stack<'p> {
    bases: Vec<&'p Package>,
    top: &'p Package,
}

impl<'p> Stack<'p> {
    /// Resolves the bases of `top`, and theirs, among `pool`, depth first, so a
    /// package comes after every package it builds on and a shared base
    /// appears once. Stacks are a handful of packages, so the lookups are linear.
    pub(in crate::spec_load) fn resolve(
        top: &'p Package,
        pool: &'p [Package],
    ) -> Result<Stack<'p>, SpecError> {
        let mut bases = Vec::new();
        visit_bases(top, pool, &mut bases, &mut vec![top.id()])
            .map_err(|reason| SpecError::Stack { reason })?;
        Ok(Stack { bases, top })
    }

    /// The packages of the stack, bases first, the top last.
    pub(in crate::spec_load) fn steps(&self) -> impl Iterator<Item = &'p Package> + '_ {
        self.bases.iter().copied().chain([self.top])
    }

    /// The package the stack is built for.
    pub(in crate::spec_load) fn top(&self) -> &'p Package {
        self.top
    }

    /// The packages the top builds on, bases first.
    pub(in crate::spec_load) fn bases(&self) -> &[&'p Package] {
        &self.bases
    }
}

// Places each base of `package` after the bases it builds on. `path` holds the
// ids from the top down to `package`, so a base already on it closes a circle.
fn visit_bases<'p>(
    package: &'p Package,
    pool: &'p [Package],
    placed: &mut Vec<&'p Package>,
    path: &mut Vec<&'p PackageId>,
) -> Result<(), Box<StackError>> {
    for id in &package.header().bases {
        if let Some(start) = path.iter().position(|member| *member == id) {
            let members = path[start..].iter().map(|&member| member.clone()).collect();
            return Err(Box::new(StackError::BaseCycle { members }));
        }
        if placed.iter().any(|base| base.id() == id) {
            continue;
        }
        let base = pool
            .iter()
            .find(|candidate| candidate.id() == id)
            .ok_or_else(|| {
                Box::new(StackError::UnknownBase {
                    package: Box::new(package.id().clone()),
                    base: Box::new(id.clone()),
                })
            })?;
        path.push(base.id());
        visit_bases(base, pool, placed, path)?;
        path.pop();
        placed.push(base);
    }
    Ok(())
}
