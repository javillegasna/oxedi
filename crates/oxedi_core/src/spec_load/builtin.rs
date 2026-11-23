//! The specs the core ships, found by transaction set and version or by package identity.

use std::fmt;
use std::sync::{Arc, OnceLock};

use serde_json::{Value, json};

use super::model::SpecModel;
use super::package::{PackageId, Release, TransactionSet};
use super::{LoadedSpec, SpecError, merge_patch};
use crate::message::render_names;
use crate::segment::Segment;
use crate::spec::{Spec, chosen};

/// An X12 release a built-in spec covers. It is not specific to one
/// transaction set: other sets share the releases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Version {
    /// 5010.
    V5010,
    /// 4010.
    V4010,
}

impl Version {
    /// Every version, in the order of the registry.
    pub const ALL: [Version; 2] = [Version::V5010, Version::V4010];

    /// The release as `version` options write it, e.g. `"5010"`.
    pub const fn text(self) -> &'static str {
        match self {
            Version::V5010 => "5010",
            Version::V4010 => "4010",
        }
    }

    /// The release word of the id's release this version covers, e.g. `"005010"`.
    pub const fn release(self) -> &'static str {
        match self {
            Version::V5010 => "005010",
            Version::V4010 => "004010",
        }
    }

    /// The version whose release word is `release`, if there is one.
    pub fn of_release(release: &Release) -> Option<Version> {
        Version::ALL
            .into_iter()
            .find(|version| version.release() == release.as_str())
    }

    /// The version `text` names, if there is one.
    pub fn parse(text: &str) -> Option<Version> {
        Version::ALL
            .into_iter()
            .find(|version| version.text() == text)
    }
}

/// One spec the core ships, found by the identity of its top package, or by the
/// transaction set and version that identity names.
#[derive(Debug)]
pub struct Builtin {
    /// The identity of the built-in's top package.
    pub id: PackageId,
    /// The transaction set the id names, e.g. `835`, read from it once.
    pub transaction: TransactionSet,
    /// The version of the id's release, read from it once.
    pub version: Version,
    /// The spec.
    pub spec: Arc<Spec>,
    chain: &'static [&'static str],
}

impl Builtin {
    /// The built-in's JSON: its package stack composed into one value.
    #[cfg(test)]
    pub(in crate::spec_load) fn json(&self) -> Result<Value, SpecError> {
        merged(self.chain, self.id.spec_name())
    }

    /// The spec's model, composed now from the packages the built-in ships;
    /// the registry holds no model itself.
    pub fn model(&self) -> Result<SpecModel, SpecError> {
        composed(self.chain, self.id.spec_name())
    }

    /// The built-in as a loader keeps it: its model, read now, beside its compiled spec.
    pub fn loaded(&self) -> Result<LoadedSpec, SpecError> {
        self.model().map(|model| LoadedSpec {
            model,
            spec: Arc::clone(&self.spec),
        })
    }
}

impl Spec {
    /// The built-in 835 structure.
    pub fn builtin_835() -> Spec {
        Spec::clone(&Builtin::fallback().spec)
    }

    /// The built-in 835 for version 4010: its edition package composed over the
    /// 5010 one.
    pub fn builtin_835_4010() -> Spec {
        Builtin::find(Builtin::fallback().transaction.as_str(), Version::V4010)
            .map(|builtin| Spec::clone(&builtin.spec))
            .expect(
                "the registry holds the 835 at 4010; spec_load::tests::builtin::find_835_4010_gives_the_4010_patch checks it",
            )
    }
}

/// Every package the crate ships: the 5010 stack, then the 4010's.
pub(in crate::spec_load) const PACKAGES: [&str; 3] = [
    include_str!("../../specs/oxedi/envelope/005010.json"),
    include_str!("../../specs/835.json"),
    include_str!("../../specs/oxedi/guide/004010/835/X091A1.json"),
];

/// The packages of the 5010 stack, the first of `PACKAGES`.
const FIVE: usize = 2;

/// The package texts of each built-in, bases first and the top last; the
/// fallback comes first.
const BUILTINS: [&[&str]; 2] = [PACKAGES.split_at(FIVE).0, &PACKAGES];

// The value a built-in is: the spec of each package merged in order under the
// name the top package's id gives. The packages' headers are not checked here;
// `spec_load::tests::builtin::the_lean_read_equals_the_checked_composition`
// proves this read gives what the checked composer gives.
fn merged(chain: &[&str], name: &str) -> Result<Value, SpecError> {
    let packages = chain
        .iter()
        .enumerate()
        .map(|(position, text)| {
            serde_json::from_str::<Value>(text).map_err(|error| {
                shipped_fault(format_args!(
                    "shipped package {} of {} is not JSON: {error}",
                    position + 1,
                    chain.len()
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut value = json!({ "name": name });
    for (position, package) in packages.iter().enumerate() {
        let body = package.get("spec").ok_or_else(|| {
            shipped_fault(format_args!(
                "shipped package {} of {} has no \"spec\"",
                position + 1,
                packages.len()
            ))
        })?;
        merge_patch(&mut value, body);
    }
    Ok(value)
}

fn shipped_fault(reason: impl fmt::Display) -> SpecError {
    SpecError::Format {
        path: ".".to_owned(),
        source: <serde_json::Error as serde::de::Error>::custom(reason),
    }
}

fn composed(chain: &[&str], name: &str) -> Result<SpecModel, SpecError> {
    merged(chain, name).and_then(|value| SpecModel::from_value(&value))
}

// The identity the last package of `chain` declares.
fn top_id(chain: &[&str]) -> Option<PackageId> {
    let top: Value = serde_json::from_str(chain.last()?).ok()?;
    PackageId::parse(top.pointer("/package/id")?.as_str()?).ok()
}

const KEYS: &str = "a built-in's id names a transaction set and a known release; \
    spec_load::tests::builtin::every_builtin_takes_its_keys_from_its_id checks it";

// Each built-in compiles from its packages merged by value, and takes its
// transaction set and version from the id of its top package; no model is
// written back to JSON here.
fn load() -> Vec<Builtin> {
    BUILTINS
        .into_iter()
        .map(|chain| {
            let id = top_id(chain).expect(KEYS);
            let transaction = id.transaction_set().cloned().expect(KEYS);
            let version = Version::of_release(id.release()).expect(KEYS);
            let spec = composed(chain, id.spec_name())
                .and_then(|model| model.compile())
                .expect(
                    "a built-in spec is valid; \
                     spec_load::tests::builtin::each_builtin_spec_is_compiled_from_its_model checks it",
                );
            Builtin {
                id,
                transaction,
                version,
                spec: Arc::new(spec),
                chain,
            }
        })
        .collect()
}

static ALL: OnceLock<Vec<Builtin>> = OnceLock::new();

impl Builtin {
    /// The version of the built-in [`Builtin::fallback`] returns.
    pub const DEFAULT_VERSION: Version = Version::V5010;

    /// Every built-in, loaded once per process; the first is the fallback.
    pub fn all() -> &'static [Builtin] {
        ALL.get_or_init(load)
    }

    /// The built-in used when a file declares no version a built-in covers.
    pub fn fallback() -> &'static Builtin {
        &Builtin::all()[0]
    }

    /// The built-in whose top package is `id`.
    pub fn get(id: &PackageId) -> Option<&'static Builtin> {
        Builtin::all().iter().find(|b| &b.id == id)
    }

    /// The built-in of `transaction` at `version`.
    pub fn find(transaction: &str, version: Version) -> Result<&'static Builtin, UnknownVersion> {
        Builtin::all()
            .iter()
            .find(|b| b.transaction.as_str() == transaction && b.version == version)
            .ok_or_else(|| UnknownVersion::new(transaction, version.text()))
    }

    /// The built-in of `transaction` at the version `text` names.
    pub fn find_text(transaction: &str, text: &str) -> Result<&'static Builtin, UnknownVersion> {
        Version::parse(text)
            .ok_or_else(|| UnknownVersion::new(transaction, text))
            .and_then(|version| Builtin::find(transaction, version))
    }

    /// The versions of `transaction`'s built-ins, in registry order.
    pub fn versions(transaction: &str) -> Vec<Version> {
        Builtin::all()
            .iter()
            .filter(|b| b.transaction.as_str() == transaction)
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
    /// The versions the transaction's built-ins cover, as text; empty when
    /// no built-in covers the transaction.
    pub known: Vec<&'static str>,
}

impl UnknownVersion {
    fn new(transaction: &str, version: &str) -> UnknownVersion {
        UnknownVersion {
            transaction: transaction.to_owned(),
            version: version.to_owned(),
            known: Builtin::versions(transaction)
                .into_iter()
                .map(Version::text)
                .collect(),
        }
    }
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
        write!(f, "version must be one of {}", render_names(&self.known))
    }
}

impl std::error::Error for UnknownVersion {}
