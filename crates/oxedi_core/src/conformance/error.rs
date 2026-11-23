//! Why a kit does not run on a stack.

use std::fmt;

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

impl fmt::Display for WrongStack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the kit checks package \"{}\", but the stack's top package is \"{}\"; run the kit \
             on the stack that has \"{}\" on top, or change the kit's \"package\" to \"{}\"",
            self.kit, self.stack, self.kit, self.stack
        )
    }
}

impl std::error::Error for WrongStack {}
