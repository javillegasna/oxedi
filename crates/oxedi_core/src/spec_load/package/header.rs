//! A package: a header around a spec of the layers it holds.

use std::collections::{BTreeMap, HashSet};
use std::num::NonZeroU32;

use serde::Deserialize;
use serde::de::{self, IgnoredAny};
use serde_json::Value;

use super::error::HeaderError;
use super::id::PackageId;
use super::layer::Layers;
use crate::spec_load::error::SpecError;
use crate::spec_load::model::format_error;
use crate::spec_load::strict::{StrictValue, newer_format};

/// The header of a package.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a package header object")]
pub struct PackageHeader {
    /// The format the package is written in.
    pub format: NonZeroU32,
    /// The package's identity.
    pub id: PackageId,
    /// The layers the package holds.
    pub layers: Layers,
    /// The packages this one builds on.
    pub bases: Vec<PackageId>,
}

impl PackageHeader {
    /// The newest format this reader takes. A new optional key raises it, and a
    /// reader takes its own number and every lower one.
    /// Format 2 lets `version` leave out `values`. Format 3 lets an envelope
    /// loop's `control` state `fields`.
    pub const FORMAT: u32 = 3;
}

/// The two keys a package file holds; the spec is kept as a value.
#[derive(Deserialize)]
#[serde(
    deny_unknown_fields,
    expecting = "a package object with \"package\" and \"spec\""
)]
struct Keys {
    package: PackageHeader,
    #[serde(rename = "spec")]
    _spec: BTreeMap<String, IgnoredAny>,
}

/// One package file: its header and the spec value it holds.
#[derive(Debug, Clone)]
pub struct Package {
    header: PackageHeader,
    body: Value,
}

impl Package {
    /// Reads a package. A `format` above [`PackageHeader::FORMAT`] is refused
    /// before any other key is read, so a newer package's new keys never
    /// surface as unknown fields.
    pub fn from_json(json: &str) -> Result<Package, SpecError> {
        let mut value: Value = serde_json::from_str(json).map_err(SpecError::Json)?;
        if let Some(found) = newer_format(&value, "/package/format", PackageHeader::FORMAT) {
            return Err(bad_header(HeaderError::FormatTooNew {
                found,
                reader: PackageHeader::FORMAT,
            }));
        }
        let Keys {
            package: header, ..
        } = serde_path_to_error::deserialize(StrictValue(&value)).map_err(format_error)?;
        if let Some(reason) = refuse(&header) {
            return Err(bad_header(reason));
        }
        let body = value
            .as_object_mut()
            .and_then(|keys| keys.remove("spec"))
            .ok_or_else(|| SpecError::Format {
                path: ".".to_owned(),
                source: de::Error::custom("missing field `spec`"),
            })?;
        Ok(Package { header, body })
    }

    /// The header.
    pub fn header(&self) -> &PackageHeader {
        &self.header
    }

    /// The package's identity.
    pub fn id(&self) -> &PackageId {
        &self.header.id
    }

    /// The spec value the package holds.
    pub(in crate::spec_load) fn body(&self) -> &Value {
        &self.body
    }
}

/// The first rule a header that read breaks: the id names the top layer, and
/// no base is listed twice.
fn refuse(header: &PackageHeader) -> Option<HeaderError> {
    if header.id.layer() != header.layers.top() {
        return Some(HeaderError::IdLayerNotTop {
            top: header.layers.top(),
            id: Box::new(header.id.clone()),
        });
    }
    repeated(&header.bases).map(|base| HeaderError::RepeatedBase {
        base: Box::new(base.clone()),
        package: Box::new(header.id.clone()),
    })
}

/// The first base that an earlier one in `bases` already names.
fn repeated(bases: &[PackageId]) -> Option<&PackageId> {
    let mut seen = HashSet::new();
    bases.iter().find(|base| !seen.insert(*base))
}

fn bad_header(reason: HeaderError) -> SpecError {
    SpecError::BadHeader { reason }
}
