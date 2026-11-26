//! The text of a package id, header, stack, change or kit that was refused, and of
//! a version no built-in covers.

use std::fmt;

use crate::message::render_names;
use crate::spec_load::{
    Change, ChangeKind, HeaderError, IdError, KitError, Layer, PackageId, StackError,
    UnknownVersion,
};
use crate::vocabulary::words_text;

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdError::Parts { id, form } => {
                write!(f, "package id \"{id}\" is not written {form}")
            }
            IdError::Layer { id, layer } => write!(
                f,
                "package id \"{id}\": layer \"{layer}\" must be {}",
                words_text::<Layer>()
            ),
            IdError::Part { id, part, text } => write!(
                f,
                "package id \"{id}\": {} \"{text}\" must be {}",
                part.name(),
                part.rule()
            ),
        }
    }
}

impl fmt::Display for HeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HeaderError::FormatTooNew { found, reader } => write!(
                f,
                "package format {found} is newer than {reader}, the newest format this reader \
                 takes; read the package with a newer oxedi, or write it in format {reader}"
            ),
            HeaderError::IdLayerNotTop { id, top } => write!(
                f,
                "package \"{id}\" names layer \"{}\" in its id, but the top layer \"layers\" \
                 lists is \"{top}\"; the id names the package's top layer",
                id.layer()
            ),
            HeaderError::RepeatedBase { package, base } => write!(
                f,
                "package \"{package}\" lists base \"{base}\" more than once in \"bases\"; \
                 list each base once"
            ),
        }
    }
}

impl fmt::Display for StackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StackError::UnknownBase { package, base } => write!(
                f,
                "package \"{package}\" builds on \"{base}\", which is not among the packages \
                 given; give that package too, or remove it from \"bases\""
            ),
            StackError::DuplicatePackage { id } => write!(
                f,
                "two of the packages given are named \"{id}\"; give each package once"
            ),
            StackError::LayerOrder {
                package,
                lowest,
                base,
                base_top,
            } => write!(
                f,
                "package \"{package}\" holds layer \"{lowest}\", which does not come after \
                 layer \"{base_top}\" of its base \"{base}\"; a package builds only on lower \
                 layers, unless it is an edition (it holds a layer its base holds) or a patch"
            ),
            StackError::KeyNotOwned {
                package,
                path,
                owners,
                layers,
            } => {
                let owners: Vec<String> = owners
                    .iter()
                    .map(|owner| format!("{:?}", owner.word()))
                    .collect();
                write!(
                    f,
                    "package \"{package}\" holds {path}, a key of layer {}, but its layers are \
                     {layers}; move the key to a package of that layer, or list the layer in \
                     \"layers\"",
                    owners.join(" or ")
                )
            }
            StackError::NameFromIdentity { package } => write!(
                f,
                "package \"{package}\" holds \"name\"; a stack takes its name from its top \
                 package's id and only a patch may change it, so remove the key"
            ),
            StackError::Widens { change } => write!(
                f,
                "package \"{}\" does not only narrow what its bases allow: {change}; a package only narrows \
                 its bases, unless it is an edition (it holds a layer its base holds) or a patch",
                change.package
            ),
            StackError::BaseCycle { members } => {
                let circle: Vec<&str> = members
                    .iter()
                    .chain(members.first())
                    .map(PackageId::as_str)
                    .collect();
                write!(
                    f,
                    "packages form a base cycle: {}; a package cannot build on itself",
                    circle.join(" -> ")
                )
            }
        }
    }
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

impl fmt::Display for KitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KitError::FormatTooNew { found, reader } => write!(
                f,
                "kit format {found} is newer than {reader}, the newest format this reader \
                 takes; read the kit with a newer oxedi, or write it in format {reader}"
            ),
            KitError::NoCases => {
                write!(f, "the kit has no case; add one under \"cases\"")
            }
            KitError::RepeatedCase { case } => write!(
                f,
                "case {case:?} is named twice under \"cases\"; rename one"
            ),
            KitError::NoOutputs { case } => write!(
                f,
                "case {case:?} expects no output; name a file under \"expect\" for \
                 \"tables\" or \"diagnostics\""
            ),
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
