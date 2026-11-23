//! One difference between a spec model and the one a package builds on.

use std::fmt;

use super::id::PackageId;

/// One difference between the model before a package and the model after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// The package that made it.
    pub package: PackageId,
    /// Where, from the spec's root, a dot between the names.
    pub path: String,
    /// What happened there.
    pub kind: ChangeKind,
}

/// How a value differs from the one of the base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    /// It allows less than before: fewer codes, a tighter length, a stricter usage.
    Narrowed {
        /// The base's value as a spec writes it.
        before: String,
        /// The package's value as a spec writes it.
        after: String,
    },
    /// It allows more than before.
    Widened {
        /// The base's value as a spec writes it.
        before: String,
        /// The package's value as a spec writes it.
        after: String,
    },
    /// It is neither narrower nor wider: a name, a type, a structure.
    Changed {
        /// The base's value as a spec writes it.
        before: String,
        /// The package's value as a spec writes it.
        after: String,
    },
    /// The base has it and the package does not.
    Removed,
    /// The package has it and the base does not.
    Added {
        /// The package's value as a spec writes it.
        after: String,
    },
}

impl fmt::Display for Change {
    /// `path: before becomes after`, `path is removed` or `path is added: after`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ChangeKind::Narrowed { before, after }
            | ChangeKind::Widened { before, after }
            | ChangeKind::Changed { before, after } => {
                write!(f, "{}: {before} becomes {after}", self.path)
            }
            ChangeKind::Removed => write!(f, "{} is removed", self.path),
            ChangeKind::Added { after } => write!(f, "{} is added: {after}", self.path),
        }
    }
}
