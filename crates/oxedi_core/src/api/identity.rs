//! The built-in a transaction set and a version name.

use crate::spec_load::{Builtin, UnknownVersion, Version};

/// A transaction set and a version, which together name a built-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity<'t> {
    /// The transaction set, as the registry writes it.
    pub transaction: &'t str,
    /// The version of that set.
    pub version: Version,
}

impl Identity<'static> {
    /// The built-in a read uses when the file declares no version a built-in
    /// covers, and a write's default: the registry's first.
    pub fn fallback() -> Self {
        Identity::of(Builtin::fallback())
    }

    /// The fallback's transaction set at `version`.
    pub fn at(version: Version) -> Self {
        Identity {
            transaction: Builtin::fallback().transaction.as_str(),
            version,
        }
    }
}

impl<'t> Identity<'t> {
    /// The identity of `builtin`.
    pub fn of(builtin: &'t Builtin) -> Self {
        Identity {
            transaction: builtin.transaction.as_str(),
            version: builtin.version,
        }
    }
}

/// The built-in `identity` names, or the version no built-in covers.
pub fn builtin(identity: Identity<'_>) -> Result<&'static Builtin, UnknownVersion> {
    Builtin::find(identity.transaction, identity.version)
}
