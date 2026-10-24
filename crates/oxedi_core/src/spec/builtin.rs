//! The specs the core ships, found by transaction set and version.

use std::fmt;
use std::sync::{Arc, OnceLock};

use super::Spec;
use super::version::chosen;
use crate::segment::Segment;

/// One spec the core ships, with the transaction set and version it covers.
#[derive(Debug)]
pub struct Builtin {
    /// The transaction set, e.g. `"835"`.
    pub transaction: &'static str,
    /// The version, e.g. `"5010"`, as `version` options name it.
    pub version: &'static str,
    /// The spec.
    pub spec: Arc<Spec>,
}

static ALL: OnceLock<Vec<Builtin>> = OnceLock::new();

impl Builtin {
    /// The version of the built-in [`Builtin::fallback`] returns.
    pub const DEFAULT_VERSION: &'static str = "5010";

    /// Every built-in, loaded once per process; the first is the fallback.
    pub fn all() -> &'static [Builtin] {
        ALL.get_or_init(|| {
            vec![
                Builtin {
                    transaction: "835",
                    version: Builtin::DEFAULT_VERSION,
                    spec: Arc::new(Spec::builtin_835()),
                },
                Builtin {
                    transaction: "835",
                    version: "4010",
                    spec: Arc::new(Spec::builtin_835_4010()),
                },
            ]
        })
    }

    /// The built-in used when a file declares no version a built-in covers.
    pub fn fallback() -> &'static Builtin {
        &Builtin::all()[0]
    }

    /// The built-in of `transaction` at `version`.
    pub fn find(transaction: &str, version: &str) -> Result<&'static Builtin, UnknownVersion> {
        Builtin::all()
            .iter()
            .find(|b| b.transaction == transaction && b.version == version)
            .ok_or_else(|| UnknownVersion {
                transaction: transaction.to_owned(),
                version: version.to_owned(),
                known: Builtin::versions(transaction),
            })
    }

    /// The versions of `transaction`'s built-ins, in registry order.
    pub fn versions(transaction: &str) -> Vec<&'static str> {
        Builtin::all()
            .iter()
            .filter(|b| b.transaction == transaction)
            .map(|b| b.version)
            .collect()
    }

    /// The built-in whose declared version `segments` carry, else the
    /// fallback; the rule of [`Spec::select`], sharing its code.
    pub fn select<'s>(segments: impl IntoIterator<Item = Segment<'s>>) -> &'static Builtin {
        let all = Builtin::all();
        let specs: Vec<&Spec> = all.iter().map(|b| b.spec.as_ref()).collect();
        chosen(&specs, segments)
            .and_then(|at| all.get(at))
            .unwrap_or_else(|| Builtin::fallback())
    }
}

/// A version no built-in covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownVersion {
    /// The transaction set asked for.
    pub transaction: String,
    /// The version asked for.
    pub version: String,
    /// The versions the transaction's built-ins cover; empty when no
    /// built-in covers the transaction.
    pub known: Vec<&'static str>,
}

impl fmt::Display for UnknownVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown version {:?}; ", self.version)?;
        if self.known.is_empty() {
            return write!(
                f,
                "no built-in spec covers transaction {:?}",
                self.transaction
            );
        }
        write!(f, "version must be one of ")?;
        for (i, known) in self.known.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{known:?}")?;
        }
        Ok(())
    }
}

impl std::error::Error for UnknownVersion {}
