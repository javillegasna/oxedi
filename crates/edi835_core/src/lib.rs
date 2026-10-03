//! Lossless, fast, data-driven EDI 835 parser core.
//!
//! Bytes in, structure out, in layers that each point back at the input:
//!
//! - [`Tokenizer`] reads the delimiters from the ISA and yields generic
//!   [`Segment`]s lazily; [`Segment::write_to`] writes them back.
//! - [`Document`] indexes every segment of a buffer once so any segment can be
//!   reached by index or byte span.
//! - [`Spec`] is a JSON loop structure (parents, triggers, held segments, end
//!   segments, envelope controls) plus the names and types of each segment's
//!   elements, and can be patched with JSON Merge Patch; [`Spec::builtin_835`]
//!   ships the 835 as such data.
//! - [`LoopEngine`] interprets a segment stream against a spec and emits
//!   [`Event`]s (loops opened and closed, segments captured or unmatched);
//!   [`LoopTree`] collects those events into a tree of loop instances.
//! - [`EnvelopeChecker`] reads the same events and reports structural
//!   [`Diagnostic`]s: unknown segments, implicit or unterminated loops, and
//!   envelope counts or control numbers that do not match.
//!
//! The structure is data: no code in this crate is specific to the 835 beyond
//! the built-in spec it loads.

pub mod check;
pub mod column;
pub mod delimiters;
pub mod diagnostic;
pub mod document;
pub mod element;
pub mod engine;
pub mod frame;
pub mod segment;
pub mod spec;
pub mod tokenizer;
pub mod tree;

pub use check::EnvelopeChecker;
pub use column::{
    Bitmap, Cell, CellError, Column, ColumnData, ColumnType, RowError, Table, Tables, parse_dt,
    parse_n, parse_r, parse_tm,
};
pub use delimiters::{Delimiters, IsaError};
pub use diagnostic::{Diagnostic, LoopRef, Rule, SnipLevel};
pub use document::{Document, Segments, Span};
pub use element::{Element, Value};
pub use engine::{Event, LoopEngine};
pub use frame::{Frame, next_frame};
pub use segment::{Segment, WriteError};
pub use spec::{
    ColumnSource, Control, ControlCount, ControlError, ElementDef, ElementDefError, ElementType,
    LoopDef, LoopId, Repeat, SegmentDef, Spec, SpecError, TableDef, TableDefError, Trigger,
    merge_patch,
};
pub use tokenizer::Tokenizer;
pub use tree::{LoopTree, Node, NodeId};
