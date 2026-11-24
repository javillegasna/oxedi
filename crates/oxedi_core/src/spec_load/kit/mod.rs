//! The conformance kit's manifest: a JSON file that names X12 inputs and the
//! outputs a spec is expected to give for each, read strictly.
//!
//! `manifest` the kit, its header, its cases and the paths they name; `error`
//! why a kit is refused once its keys read. A path is a name the caller
//! resolves: nothing here opens a file.

mod error;
mod manifest;
#[cfg(test)]
mod tests;

pub use error::KitError;
pub use manifest::{Kit, KitCase, KitHeader, KitOutput, KitPath};
