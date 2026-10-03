//! Lossless, fast, data-driven EDI 835 parser core.
//!
//! Bytes in, a lazy stream of generic segments out. Nothing in this crate
//! knows what an 835 is: segment meaning is supplied as data by higher layers.

pub mod delimiters;
pub mod document;
pub mod element;
pub mod engine;
pub mod frame;
pub mod segment;
pub mod spec;
pub mod tokenizer;

pub use delimiters::{Delimiters, IsaError};
pub use document::{Document, Segments, Span};
pub use element::{Element, Value};
pub use engine::{Event, LoopEngine};
pub use frame::{Frame, next_frame};
pub use segment::{Segment, WriteError};
pub use spec::{LoopDef, LoopId, Spec, SpecError, Trigger, merge_patch};
pub use tokenizer::Tokenizer;
