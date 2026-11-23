//! The package: a JSON file that holds a spec of some layers under a header,
//! and the identity that names it.
//!
//! `layer` the layers a vocabulary is written in, in layer order; `id` the
//! identity of a package and its grammar; `error` why an identity is rejected.

mod error;
mod id;
mod layer;
#[cfg(test)]
mod tests;

pub use error::IdError;
pub use id::{Guide, IdPart, PackageId, Release, Scope, TransactionSet};
pub use layer::Layer;
