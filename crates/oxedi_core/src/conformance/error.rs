//! Why a kit does not run on a stack.

use crate::spec_load::PackageId;

/// The kit checks another package than the stack's top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrongStack {
    /// The package the kit's header names, boxed to keep the error small.
    pub kit: Box<PackageId>,
    /// The top package of the stack given to the runner, boxed to keep the
    /// error small.
    pub stack: Box<PackageId>,
}

impl std::error::Error for WrongStack {}
