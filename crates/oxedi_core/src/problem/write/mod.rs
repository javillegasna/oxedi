//! The text of the writer's problems.
//!
//! The folder holds: `refusal.rs` (why a spec cannot be written, and what to
//! change about it), `finding.rs` (where a value came from, the findings about
//! written tables and the write error), `wording.rs` (the pieces those messages
//! are written from) and `tests.rs` (the pieces' text).

mod finding;
mod refusal;
mod wording;

#[cfg(test)]
mod tests;
