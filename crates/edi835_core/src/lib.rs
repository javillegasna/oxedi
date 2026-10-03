//! Lossless, fast, data-driven EDI 835 parser core.
//!
//! Bytes in, structure out, in layers that each point back at the input:
//!
//! - [`Tokenizer`] reads the delimiters from the ISA and yields generic
//!   [`Segment`]s lazily; [`Segment::write_to`] writes them back.
//! - [`Document`] indexes every segment of a buffer once so any segment can be
//!   reached by index or byte span.
//! - [`Spec`] is a JSON loop structure (parents, triggers, held segments, end
//!   segments) that can be patched with JSON Merge Patch; [`Spec::builtin_835`]
//!   ships the 835 structure as such data.
//! - [`LoopEngine`] interprets a segment stream against a spec and emits
//!   [`Event`]s (loops opened and closed, segments captured or unmatched);
//!   [`LoopTree`] collects those events into a tree of loop instances.
//!
//! The structure is data: no code in this crate is specific to the 835 beyond
//! the built-in spec it loads.

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

pub use delimiters::{Delimiters, IsaError};
pub use diagnostic::{Diagnostic, LoopRef, Rule, SnipLevel};
pub use document::{Document, Segments, Span};
pub use element::{Element, Value};
pub use engine::{Event, LoopEngine};
pub use frame::{Frame, next_frame};
pub use segment::{Segment, WriteError};
pub use spec::{
    ElementDef, ElementDefError, ElementType, LoopDef, LoopId, SegmentDef, Spec, SpecError,
    Trigger, merge_patch,
};
pub use tokenizer::Tokenizer;
pub use tree::{LoopTree, Node, NodeId};
