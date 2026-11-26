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
use crate::segment::Segment;
use crate::spec::{Spec, UnknownTable};
use crate::tree::LoopTree;

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

/// [`read`], and the loop tree of the file. The tables and diagnostics are
/// those of [`read`].
pub fn read_tree<'a, G: Deref<Target = Spec>>(
    bytes: impl Into<Cow<'a, [u8]>>,
    options: ReadOptions<'_, G>,
) -> Result<(Reading<'a, G>, LoopTree), DocumentError> {
    let document = options.delimiters.document(bytes)?;
    let spec = options.spec.choose(document.segments());
    let (tables, diagnostics, tree) = fold_tree(spec.spec(), &document, options.mask);
    let reading = Reading {
        document,
        spec,
        tables,
        diagnostics,
    };
    Ok((reading, tree))
}

/// One pass: every segment goes to the processor and its events to the tree.
fn fold_tree(
    spec: &Spec,
    document: &Document<'_>,
    mask: Option<&Mask>,
) -> (Tables, Vec<Diagnostic>, LoopTree) {
    let mut processor = Processor::new(spec, document.delimiters()).with_mask(mask);
    let mut builder = LoopTree::builder();
    let mut diagnostics = Vec::new();
    // One segment buffer for the whole pass, as `Processor::run` keeps.
    let mut segment = Segment::empty();
    let mut index = 0;
    while document.segment_into(index, &mut segment) {
        let output = processor.feed(&segment);
        builder.on(output.events());
        diagnostics.extend_from_slice(output.diagnostics());
        index += 1;
    }
    let output = processor.finish();
    builder.on(output.events());
    diagnostics.extend_from_slice(output.diagnostics());
    (processor.take_tables(), diagnostics, builder.finish())
}
