//! Which spec a read uses and where its delimiters come from.

use std::borrow::Cow;
use std::ops::Deref;
use std::sync::Arc;

use crate::delimiters::{Delimiters, IsaError};
use crate::document::{Document, DocumentError};
use crate::segment::Segment;
use crate::spec::Spec;
use crate::spec_load::Builtin;
use crate::tokenizer::Tokenizer;

/// How a read gets its spec; `G` is the type that holds a given spec.
#[derive(Debug, Clone)]
pub enum SpecChoice<G = Arc<Spec>> {
    /// The built-in of the version the file declares, else the default.
    Declared,
    /// This spec, with the file's version unread.
    Given(G),
}

#[expect(clippy::derivable_impls, reason = "a derive would require G: Default")]
impl<G> Default for SpecChoice<G> {
    fn default() -> Self {
        SpecChoice::Declared
    }
}

/// The spec a read settled on.
#[derive(Debug, Clone)]
pub enum ChosenSpec<G = Arc<Spec>> {
    /// The built-in [`Builtin::select`] gives for the file.
    Builtin(&'static Builtin),
    /// The spec the caller gave.
    Given(G),
}

impl<G> SpecChoice<G> {
    /// Settles the choice over the file's `segments`. Only `Declared` reads
    /// them (for the version the file declares).
    pub fn choose<'s>(self, segments: impl IntoIterator<Item = Segment<'s>>) -> ChosenSpec<G> {
        match self {
            SpecChoice::Declared => ChosenSpec::Builtin(Builtin::select(segments)),
            SpecChoice::Given(given) => ChosenSpec::Given(given),
        }
    }
}

impl<G: Deref<Target = Spec>> ChosenSpec<G> {
    /// The spec itself.
    pub fn spec(&self) -> &Spec {
        match self {
            ChosenSpec::Builtin(builtin) => &builtin.spec,
            ChosenSpec::Given(given) => given,
        }
    }
}

/// Where a read gets its delimiters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DelimiterChoice {
    /// From the file's ISA segment.
    #[default]
    FromHeader,
    /// These delimiters, for a file with no ISA.
    Given(Delimiters),
}

impl DelimiterChoice {
    /// Indexes `bytes` with the chosen delimiters.
    pub fn document<'a>(
        self,
        bytes: impl Into<Cow<'a, [u8]>>,
    ) -> Result<Document<'a>, DocumentError> {
        match self {
            DelimiterChoice::FromHeader => Document::parse(bytes),
            DelimiterChoice::Given(delimiters) => {
                Document::with_delimiters(bytes, delimiters).map_err(DocumentError::Size)
            }
        }
    }

    /// A segment reader over `bytes` with the chosen delimiters.
    pub fn tokenizer(self, bytes: &[u8]) -> Result<Tokenizer<'_>, IsaError> {
        match self {
            DelimiterChoice::FromHeader => Tokenizer::new(bytes),
            DelimiterChoice::Given(delimiters) => Ok(Tokenizer::with_delimiters(bytes, delimiters)),
        }
    }
}
