//! The reads over a whole file.

use std::borrow::Cow;
use std::ops::Deref;
use std::sync::Arc;

use super::choice::{ChosenSpec, DelimiterChoice, SpecChoice};
use crate::column::Tables;
use crate::diagnostic::Diagnostic;
use crate::document::{Document, DocumentError};
use crate::mask::Mask;
use crate::process::Processor;
use crate::spec::{Spec, UnknownTable};

/// What a read takes besides the bytes.
#[derive(Debug, Clone)]
pub struct ReadOptions<'m, G = Arc<Spec>> {
    /// Which spec the file is read with.
    pub spec: SpecChoice<G>,
    /// Where the delimiters come from.
    pub delimiters: DelimiterChoice,
    /// The mask applied to the tables and the diagnostics; `None` is no mask.
    pub mask: Option<&'m Mask>,
}

impl<G> Default for ReadOptions<'_, G> {
    fn default() -> Self {
        Self {
            spec: SpecChoice::Declared,
            delimiters: DelimiterChoice::FromHeader,
            mask: None,
        }
    }
}

/// What a read gives.
#[derive(Debug)]
pub struct Reading<'a, G = Arc<Spec>> {
    /// The indexed file.
    pub document: Document<'a>,
    /// The spec it was read with.
    pub spec: ChosenSpec<G>,
    /// The spec's tables, filled.
    pub tables: Tables,
    /// Every diagnostic, in stream order.
    pub diagnostics: Vec<Diagnostic>,
}

/// Indexes `bytes`, chooses the spec and reads the file with it: the tables
/// and the diagnostics of [`Processor::run`].
pub fn read<'a, G: Deref<Target = Spec>>(
    bytes: impl Into<Cow<'a, [u8]>>,
    options: ReadOptions<'_, G>,
) -> Result<Reading<'a, G>, DocumentError> {
    let document = options.delimiters.document(bytes)?;
    let spec = options.spec.choose(document.segments());
    let (tables, diagnostics) = Processor::run(spec.spec(), &document, options.mask);
    Ok(Reading {
        document,
        spec,
        tables,
        diagnostics,
    })
}

/// Reads an indexed `document` with a `spec` already chosen, filling only the
/// tables named in `names` and collecting no diagnostics. A name the spec does
/// not define is an [`UnknownTable`].
pub fn read_tables(
    document: &Document<'_>,
    spec: &Spec,
    names: &[&str],
    mask: Option<&Mask>,
) -> Result<Tables, UnknownTable> {
    Processor::run_tables(spec, document, names, mask)
}
