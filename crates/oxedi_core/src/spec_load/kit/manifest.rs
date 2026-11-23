//! The kit manifest and its strict read.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use serde::Deserialize;
use serde::de::{self, Deserializer};
use serde_json::Value;

use super::error::KitError;
use crate::spec_load::error::SpecError;
use crate::spec_load::model::format_error;
use crate::spec_load::package::PackageId;
use crate::spec_load::strict::{StrictValue, newer_format};
use crate::vocabulary::{Vocabulary, serde_by_word};

/// The header of a kit.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a kit header object")]
pub struct KitHeader {
    /// The format the kit is written in.
    pub format: NonZeroU32,
    /// The top package of the stack the kit checks; the caller composes it.
    pub package: PackageId,
}

impl KitHeader {
    /// The newest format this reader takes. A new optional key raises it, and a
    /// reader takes its own number and every lower one.
    pub const FORMAT: u32 = 1;
}

/// An output of a run a case can expect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum KitOutput {
    /// The tables, as the `Display` of [`Tables`](crate::column::Tables) writes them.
    Tables,
    /// The diagnostics, one `Display` line each.
    Diagnostics,
}

impl Vocabulary for KitOutput {
    const KEY: &'static str = "output";
    const ALL: &'static [KitOutput] = &[KitOutput::Tables, KitOutput::Diagnostics];

    fn word(self) -> &'static str {
        match self {
            KitOutput::Tables => "tables",
            KitOutput::Diagnostics => "diagnostics",
        }
    }
}

serde_by_word!(KitOutput);

/// A path a kit names: relative, written with `/`, no empty part. `..` is
/// allowed. The core never opens it: a path is a key into the files the
/// caller holds, and a caller that resolves it against a directory keeps the
/// result under a root it trusts.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct KitPath(String);

impl KitPath {
    /// The path as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for KitPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let found = String::deserialize(deserializer)?;
        if found.contains('\\') || found.split('/').any(str::is_empty) {
            return Err(de::Error::custom(format!(
                "a path must be relative and written with \"/\", with no empty part, \
                 e.g. \"fixtures/a.835\"; found {found:?}"
            )));
        }
        Ok(KitPath(found))
    }
}

/// One case: an X12 input and the outputs a run on it is expected to give.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a kit case object")]
pub struct KitCase {
    /// The X12 file.
    pub input: KitPath,
    /// The file that holds each expected output.
    pub expect: BTreeMap<KitOutput, KitPath>,
}

/// The two keys a kit file holds.
#[derive(Deserialize)]
#[serde(
    deny_unknown_fields,
    expecting = "a kit object with \"kit\" and \"cases\""
)]
struct Keys {
    kit: KitHeader,
    cases: BTreeMap<String, KitCase>,
}

/// A conformance kit: a header and its cases, in name order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kit {
    header: KitHeader,
    cases: BTreeMap<String, KitCase>,
}

impl Kit {
    /// Reads a kit. A `format` above [`KitHeader::FORMAT`] is refused before
    /// any other key is read, so a newer kit's new keys never surface as
    /// unknown fields.
    pub fn from_json(json: &str) -> Result<Kit, SpecError> {
        let value: Value = serde_json::from_str(json).map_err(SpecError::Json)?;
        if let Some(found) = newer_format(&value, "/kit/format", KitHeader::FORMAT) {
            return Err(bad_kit(KitError::FormatTooNew {
                found,
                reader: KitHeader::FORMAT,
            }));
        }
        let Keys { kit: header, cases } =
            serde_path_to_error::deserialize(StrictValue(&value)).map_err(format_error)?;
        match refuse(&cases) {
            Some(reason) => Err(bad_kit(reason)),
            None => Ok(Kit { header, cases }),
        }
    }

    /// The header.
    pub fn header(&self) -> &KitHeader {
        &self.header
    }

    /// The cases by name.
    pub fn cases(&self) -> &BTreeMap<String, KitCase> {
        &self.cases
    }

    /// Every input and expected path, sorted, each once.
    pub fn files(&self) -> Vec<&str> {
        let paths: BTreeSet<&str> = self
            .cases
            .values()
            .flat_map(|case| std::iter::once(&case.input).chain(case.expect.values()))
            .map(KitPath::as_str)
            .collect();
        paths.into_iter().collect()
    }
}

/// The first rule a kit that read breaks: it holds a case, and each case
/// expects an output.
fn refuse(cases: &BTreeMap<String, KitCase>) -> Option<KitError> {
    if cases.is_empty() {
        return Some(KitError::NoCases);
    }
    cases
        .iter()
        .find(|(_, case)| case.expect.is_empty())
        .map(|(name, _)| KitError::NoOutputs { case: name.clone() })
}

fn bad_kit(reason: KitError) -> SpecError {
    SpecError::BadKit { reason }
}
