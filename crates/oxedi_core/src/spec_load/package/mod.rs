//! The package: a JSON file that holds a spec of some layers under a header,
//! and the identity that names it.
//!
//! `header` the package and its header, read strictly; `layer` the layers a
//! vocabulary is written in, in layer order; `id` the identity of a package
//! and its grammar; `stack` a top package with the packages it builds on and
//! `compose` their specs merged in order; `error` why an identity, a header or
//! a stack is rejected.

// TODO: the built-in specs compose through a stack; until then only tests use it.
#[cfg_attr(not(test), expect(dead_code))]
mod compose;
mod error;
mod header;
mod id;
mod layer;
#[cfg_attr(not(test), expect(dead_code))]
mod stack;
#[cfg(test)]
mod tests;

pub use error::{HeaderError, IdError, StackError};
pub use header::{Package, PackageHeader};
pub use id::{Guide, IdPart, PackageId, Release, Scope, TransactionSet};
pub use layer::{Layer, Layers};
