//! The door a binding uses: the reads and lookups with their options and
//! defaults as typed values, and the types they take and give. A binding
//! imports from here only; the stage modules are for the core's tests,
//! benches and Rust tools.
//!
//! The folder holds: `choice.rs` (which spec and delimiters a read uses),
//! `read.rs` (the reads over a whole file), `batches.rs` (a stream cut into
//! batches), `identity.rs` (the built-in a transaction set and a version
//! name), `mask.rs` (masking an external validator's diagnostics) and
//! `tests/`.

mod batches;
mod choice;
mod identity;
mod mask;
mod read;

// This door's own reads, choices and batches.
pub use batches::{Batch, BatchBy, Batches};
pub use choice::{ChosenSpec, DelimiterChoice, SpecChoice};
pub use identity::{Identity, builtin};
pub use mask::mask_diagnostics;
pub use read::{ReadOptions, Reading, read, read_tables, read_tree};

// The errors of the checked values a binding passes in.
pub use crate::delimiters::{DelimiterLength, DelimiterRole};
pub use crate::diagnostic::UnknownLevel;
pub use crate::document::{SegmentIndex, SegmentOutOfRange};
pub use crate::value::FloatForDecimal;

// Specs: loading, composing, validating and the conformance kits.
pub use crate::conformance::{
    CaseFault, CaseReport, Difference, KitReport, Line, Outcome, run_kit,
};
pub use crate::spec::{
    ColumnSource, Control, LoopDef, LoopId, ROW_COLUMN, Repeat, Spec, TableDef, UnknownColumn,
    UnknownTable, require_column, spec_schema, table_columns,
};
pub use crate::spec_load::{
    Builtin, Change, ChangeKind, Kit, KitOutput, LoadedSpec, SpecError, UnknownVersion, Version,
};
pub use crate::validate::{Capability, Invalid, Reasons, Valid, Validation, validate};

// Reading: the document, its segments, the diagnostics and the loop tree.
pub use crate::delimiters::{Delimiters, IsaError};
pub use crate::diagnostic::{Diagnostic, LoopPath, LoopRef, Rule, SnipLevel};
pub use crate::document::{Document, DocumentError};
pub use crate::element::Element;
pub use crate::engine::loop_paths;
pub use crate::mask::{Mask, MaskError};
pub use crate::segment::Segment;
pub use crate::tokenizer::Tokenizer;
pub use crate::tree::{LoopTree, Node, NodeId};
pub use crate::trivia::LeadingTrivia;
pub use crate::vocabulary::DateMode;

// Tables: the columns and the checked conversions into their cells.
pub use crate::column::{Cell, CellError, Column, ColumnData, RowError, Table, Tables};
pub use crate::summary::{SummaryError, SummaryValue, summarize};
pub use crate::value::{
    ColumnType, ConvertError, TimeUnit, date_from_midnight, date_from_millis, decimal_display,
    integer_from_f64, integer_from_i128, integer_from_u128, rescale, seconds_from,
};

// Writing: the envelope and the findings.
pub use crate::write::{Envelope, Finding, ValueOrigin, WriteError, write, write_with_findings};

// What a problem says: its shape and its record.
pub use crate::problem::{Detail, Place, Problem, ProblemRecord, SpecPlace};

#[cfg(test)]
mod tests;
