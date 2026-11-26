//! The door a binding uses: the reads and lookups with their options and
//! defaults as typed values, and the types they take and give. A binding
//! imports from here only; the stage modules are for the core's tests,
//! benches and Rust tools.
//!
//! The folder holds: `choice.rs` (which spec and delimiters a read uses),
//! `read.rs` (the reads over a whole file), `batches.rs` (a stream cut into
//! batches) and `tests/`.

mod batches;
mod choice;
mod read;

pub use batches::{Batch, BatchBy, Batches};
pub use choice::{ChosenSpec, DelimiterChoice, SpecChoice};
pub use read::{ReadOptions, Reading, read, read_tables, read_tree};

#[cfg(test)]
mod tests;
