//! The package: a JSON file that holds a spec of some layers under a header,
//! and the identity that names it.
//!
//! `header` the package and its header, read strictly; `layer` the layers a
//! vocabulary is written in, in layer order; `id` the identity of a package
//! and its grammar; `change` one difference between two models; `stack` a top
//! package with the packages it builds on and `compose` their specs merged in
//! order, `narrow` the comparison of the model before a package with the one
//! after it, `owners` the keys each layer may write; `error` why an identity,
//! a header or a stack is rejected.

mod change;
mod compose;
mod error;
mod header;
mod id;
mod layer;
mod narrow;
mod owners;
mod stack;
#[cfg(test)]
mod tests;

pub use change::{Change, ChangeKind};
pub use compose::Composed;
pub use error::{HeaderError, IdError, StackError};
pub use header::{Package, PackageHeader};
pub use id::{Guide, IdPart, PackageId, Release, Scope, TransactionSet};
pub use layer::{Layer, Layers};
pub use stack::{Role, Stack, Step};
