//! The specs the core ships, found by transaction set and version.

use std::fmt;
use std::sync::{Arc, OnceLock};

use serde_json::{Value, json};

use super::model::SpecModel;
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

    /// The version `text` names, if there is one.
    pub fn parse(text: &str) -> Option<Version> {
        Version::ALL
            .into_iter()
            .find(|version| version.text() == text)
    }
}

/// One spec the core ships, with the transaction set and version it covers.
#[derive(Debug)]
pub struct Builtin {
    /// The transaction set, e.g. `"835"`.
    pub transaction: &'static str,
    /// The version the spec covers.
    pub version: Version,
    /// The spec.
    pub spec: Arc<Spec>,
    chain: &'static [&'static str],
}

impl Builtin {
    /// The built-in's JSON: its package stack composed into one value.
    #[cfg(test)]
    pub(in crate::spec_load) fn json(&self) -> Result<Value, SpecError> {
        merged(self.chain)
    }

    /// The spec's model, composed now from the packages the built-in ships;
    /// the registry holds no model itself.
    pub fn model(&self) -> Result<SpecModel, SpecError> {
        composed(self.chain)
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
        Builtin::find("835", Builtin::DEFAULT_VERSION)
            .map(|builtin| Spec::clone(&builtin.spec))
            .expect(
                "the registry holds the 835 at its default version; spec_load::tests::loading::builtin_835_loads checks it",
            )
    }

    /// The built-in 835 for version 4010: its edition package composed over the
    /// 5010 one.
    pub fn builtin_835_4010() -> Spec {
        Builtin::find("835", Version::V4010)
            .map(|builtin| Spec::clone(&builtin.spec))
            .expect(
                "the registry holds the 835 at 4010; spec_load::tests::builtin::find_835_4010_gives_the_4010_patch checks it",
            )
    }
}

/// Every package the crate ships; a built-in's stack resolves among them.
pub(in crate::spec_load) const PACKAGES: [&str; 2] = [
    include_str!("../../specs/835.json"),
    include_str!("../../specs/835.4010.json"),
];

/// The transaction set, version and package texts of each built-in, bases first
/// and the top last; the fallback comes first.
const SHIPPED: [(&str, Version, &[&str]); 2] = [
    ("835", Builtin::DEFAULT_VERSION, &[PACKAGES[0]]),
    ("835", Version::V4010, &[PACKAGES[0], PACKAGES[1]]),
];

// The value a built-in is: the spec of each package merged in order under the
// name of the top package's id. The packages' headers are not checked here;
// `spec_load::tests::builtin::the_lean_read_equals_the_checked_composition`
// proves this read gives what the checked composer gives.
fn merged(chain: &[&str]) -> Result<Value, SpecError> {
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
    let name = packages
        .last()
        .and_then(|top| top.pointer("/package/id"))
        .and_then(Value::as_str)
        .and_then(spec_name)
        .ok_or_else(|| {
            shipped_fault(format_args!(
                "shipped package {n} of {n} (the top) has no id that names a spec",
                n = packages.len()
            ))
        })?;
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

// The name of a stack topped by the id `author/layer/release[/set[/guide]]`: its
// transaction set, else its release, as `PackageId::spec_name` says.
fn spec_name(id: &str) -> Option<&str> {
    let mut parts = id.split('/').skip(2);
    let release = parts.next()?;
    Some(parts.next().unwrap_or(release))
}

fn shipped_fault(reason: impl fmt::Display) -> SpecError {
    SpecError::Format {
        path: ".".to_owned(),
        source: <serde_json::Error as serde::de::Error>::custom(reason),
    }
}

fn composed(chain: &[&str]) -> Result<SpecModel, SpecError> {
    merged(chain).and_then(|value| SpecModel::from_value(&value))
}

// Each built-in compiles from its packages merged by value; no model is
// written back to JSON here.
fn load() -> Vec<Builtin> {
    SHIPPED
        .into_iter()
        .map(|(transaction, version, chain)| {
            let spec = composed(chain)
                .and_then(|model| model.compile())
                .expect(
                    "a built-in spec is valid; \
                     spec_load::tests::builtin::each_builtin_spec_is_compiled_from_its_model checks it",
                );
            Builtin {
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

    /// The built-in of `transaction` at `version`.
    pub fn find(transaction: &str, version: Version) -> Result<&'static Builtin, UnknownVersion> {
        Builtin::all()
            .iter()
            .find(|b| b.transaction == transaction && b.version == version)
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
