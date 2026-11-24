//! A stack of packages: a top package and everything it builds on.

use std::collections::HashSet;

use super::error::StackError;
use super::header::Package;
use super::id::PackageId;
use super::layer::Layer;
use crate::spec_load::error::SpecError;

/// How a package relates to its bases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// It holds layers above every base's.
    Narrows,
    /// It holds a layer one of the packages it builds on holds.
    Edition,
    /// It holds the patch layer.
    Patch,
}

impl Role {
    /// The role as a message words it: `narrows`, `edition` or `patch`.
    pub const fn word(self) -> &'static str {
        match self {
            Role::Narrows => "narrows",
            Role::Edition => "edition",
            Role::Patch => "patch",
        }
    }
}

/// One package of a stack and its role.
#[derive(Debug, Clone, Copy)]
pub struct Step<'p> {
    /// The package.
    pub package: &'p Package,
    /// How it relates to its bases.
    pub role: Role,
}

/// A top package and every package it builds on, bases first, each once.
#[derive(Debug, Clone)]
pub struct Stack<'p> {
    bases: Vec<Step<'p>>,
    top: Step<'p>,
}

impl<'p> Stack<'p> {
    /// Resolves the bases of `top`, and theirs, among `pool`, depth first, so a
    /// package comes after every package it builds on and a shared base
    /// appears once. Stacks are a handful of packages, so the lookups are linear.
    pub fn resolve(top: &'p Package, pool: &'p [Package]) -> Result<Stack<'p>, SpecError> {
        if let Some(id) = repeated_id(pool) {
            return Err(SpecError::Stack {
                reason: Box::new(StackError::DuplicatePackage {
                    id: Box::new(id.clone()),
                }),
            });
        }
        let mut bases = Vec::new();
        let role = visit_bases(top, pool, &mut bases, &mut vec![top.id()])
            .map_err(|reason| SpecError::Stack { reason })?;
        Ok(Stack {
            bases,
            top: Step { package: top, role },
        })
    }

    /// The steps of the stack, bases first, the top last.
    pub fn steps(&self) -> impl Iterator<Item = Step<'p>> + '_ {
        self.bases.iter().copied().chain([self.top])
    }

    /// The package the stack is built for.
    pub fn top(&self) -> &'p Package {
        self.top.package
    }

    /// The steps the top builds on, bases first.
    pub fn bases(&self) -> &[Step<'p>] {
        &self.bases
    }

    /// The last step: the top package and its role.
    pub fn top_step(&self) -> Step<'p> {
        self.top
    }
}

/// The first id that an earlier package of `pool` already carries.
fn repeated_id(pool: &[Package]) -> Option<&PackageId> {
    let mut seen = HashSet::new();
    pool.iter().map(Package::id).find(|id| !seen.insert(*id))
}

// Places each base of `package` after the bases it builds on and returns the
// role of `package`. `path` holds the ids from the top down to `package`, so a
// base already on it closes a circle.
fn visit_bases<'p>(
    package: &'p Package,
    pool: &'p [Package],
    placed: &mut Vec<Step<'p>>,
    path: &mut Vec<&'p PackageId>,
) -> Result<Role, Box<StackError>> {
    let mut found = Vec::new();
    for id in &package.header().bases {
        if let Some(start) = path.iter().position(|member| *member == id) {
            let members = path[start..].iter().map(|&member| member.clone()).collect();
            return Err(Box::new(StackError::BaseCycle { members }));
        }
        let base = find_base(package, id, pool)?;
        found.push(base);
        if placed.iter().any(|step| step.package.id() == id) {
            continue;
        }
        path.push(base.id());
        let role = visit_bases(base, pool, placed, path)?;
        path.pop();
        placed.push(Step {
            package: base,
            role,
        });
    }
    role_of(package, &found, &lineage(&found, pool))
}

fn find_base<'p>(
    package: &Package,
    id: &PackageId,
    pool: &'p [Package],
) -> Result<&'p Package, Box<StackError>> {
    pool.iter()
        .find(|candidate| candidate.id() == id)
        .ok_or_else(|| {
            Box::new(StackError::UnknownBase {
                package: Box::new(package.id().clone()),
                base: Box::new(id.clone()),
            })
        })
}

/// `bases` and every package they build on, each once; a base missing from
/// `pool` is skipped, since the walk that found `bases` already refused it.
fn lineage<'p>(bases: &[&'p Package], pool: &'p [Package]) -> Vec<&'p Package> {
    let mut seen: Vec<&'p Package> = Vec::new();
    let mut pending = bases.to_vec();
    while let Some(package) = pending.pop() {
        if seen.iter().any(|known| known.id() == package.id()) {
            continue;
        }
        seen.push(package);
        let parents = package.header().bases.iter();
        pending.extend(parents.filter_map(|id| pool.iter().find(|p| p.id() == id)));
    }
    seen
}

// A patch holds the patch layer; an edition shares a layer with any package it
// builds on, directly or not; any other package narrows, and must hold only
// layers above its direct bases' tops.
fn role_of(
    package: &Package,
    bases: &[&Package],
    lineage: &[&Package],
) -> Result<Role, Box<StackError>> {
    let layers = package.header().layers;
    if layers.contains(Layer::Patch) {
        return Ok(Role::Patch);
    }
    if lineage
        .iter()
        .any(|base| base.header().layers.shares(layers))
    {
        return Ok(Role::Edition);
    }
    match bases
        .iter()
        .find(|base| base.header().layers.top() >= layers.lowest())
    {
        None => Ok(Role::Narrows),
        Some(base) => Err(Box::new(StackError::LayerOrder {
            package: Box::new(package.id().clone()),
            lowest: layers.lowest(),
            base: Box::new(base.id().clone()),
            base_top: base.header().layers.top(),
        })),
    }
}
