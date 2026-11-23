//! The package: a JSON file that holds a spec of some layers under a header,
//! and the identity that names it.
//!
//! `header` the package and its header, read strictly; `layer` the layers a
//! vocabulary is written in, in layer order; `id` the identity of a package
//! and its grammar; `error` why an identity or a header is rejected.

mod error;
mod header;
mod id;
mod layer;
#[cfg(test)]
mod tests;

pub use error::{HeaderError, IdError};
pub use header::{Package, PackageHeader};
pub use id::{Guide, IdPart, PackageId, Release, Scope, TransactionSet};
pub use layer::{Layer, Layers};
