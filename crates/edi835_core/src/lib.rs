//! Lossless, fast, data-driven EDI 835 parser core.
//!
//! Bytes in, a lazy stream of generic segments out. Nothing in this crate
//! knows what an 835 is: segment meaning is supplied as data by higher layers.

pub mod delimiters;

pub use delimiters::{Delimiters, IsaError};
