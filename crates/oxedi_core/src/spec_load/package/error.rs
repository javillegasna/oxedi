//! Why a package's identity, header or stack is rejected.

use std::fmt;

use super::change::Change;
use super::id::{IdPart, PackageId};
use super::layer::{Layer, Layers};
use crate::vocabulary::words_text;

/// Why an id does not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdError {
    /// The id has fewer parts than its layer's form, or more.
    Parts {
        /// The id as written.
        id: String,
        /// The form the id should have, e.g. `<author>/envelope/<release>`.
        form: &'static str,
    },
    /// The second part is not a layer word.
    Layer {
        /// The id as written.
        id: String,
        /// The second part as written.
        layer: String,
    },
    /// A part breaks its own rule.
    Part {
        /// The id as written.
        id: String,
        /// Which part.
        part: IdPart,
        /// The part as written.
        text: String,
    },
}

impl std::error::Error for IdError {}

/// Why a package header is refused once its keys read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderError {
    /// The header's `format` is newer than the reader's.
    FormatTooNew {
        /// The format as written.
        found: u64,
        /// The newest format this reader takes.
        reader: u32,
    },
    /// The id names another layer than the top one `layers` lists.
    IdLayerNotTop {
        /// The package's id, boxed to keep the error small.
        id: Box<PackageId>,
        /// The top layer `layers` lists.
        top: Layer,
    },
    /// `bases` lists one package twice.
    RepeatedBase {
        /// The package whose header it is, boxed to keep the error small.
        package: Box<PackageId>,
        /// The base listed twice, boxed to keep the error small.
        base: Box<PackageId>,
    },
}

impl std::error::Error for HeaderError {}

/// Why a stack of packages is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackError {
    /// A package builds on a package that is not among those given.
    UnknownBase {
        /// The package that names the base, boxed to keep the error small.
        package: Box<PackageId>,
        /// The base as written, boxed to keep the error small.
        base: Box<PackageId>,
    },
    /// Two packages among those given have one id.
    DuplicatePackage {
        /// The id both packages carry, boxed to keep the error small.
        id: Box<PackageId>,
    },
    /// Packages build on each other in a circle.
    BaseCycle {
        /// The packages of the circle, from the first one reached.
        members: Vec<PackageId>,
    },
    /// A package builds on a layer it does not come after.
    LayerOrder {
        /// The package, boxed to keep the error small.
        package: Box<PackageId>,
        /// The lowest layer the package holds.
        lowest: Layer,
        /// The base whose top layer is not below it, boxed to keep the error small.
        base: Box<PackageId>,
        /// The base's top layer.
        base_top: Layer,
    },
    /// A package writes a key that none of its layers owns.
    KeyNotOwned {
        /// The package.
        package: PackageId,
        /// The key, from the spec's root, a dot between the names.
        path: String,
        /// The layers that own the key.
        owners: Vec<Layer>,
        /// The layers the package holds.
        layers: Layers,
    },
    /// A package writes the spec's `name`.
    NameFromIdentity {
        /// The package.
        package: PackageId,
    },
    /// A package that only narrows its bases changes what they allow in some other
    /// way: it allows more, or changes or removes what they hold.
    Widens {
        /// What widened and the package that did it, boxed to keep the error small.
        change: Box<Change>,
    },
}

impl std::error::Error for StackError {}
