//! The package: a JSON file that holds a spec of some layers under a header.
//!
//! `layer` the layers a vocabulary is written in, in layer order; `id` the part
//! of an identity and its grammar; `error` why a part is rejected.

mod error;
mod id;
mod layer;
#[cfg(test)]
mod tests;

pub use error::IdError;
pub use id::{Guide, IdPart, Release, TransactionSet};
pub use layer::Layer;
