//! The text of the problems a spec, a package or a kit raises when it loads.
//!
//! The folder holds: `spec.rs` (the error that wraps every other), `table.rs`,
//! `occurrence.rs`, `element.rs`, `rules.rs` (balancing rules and summaries),
//! `control.rs` (loop control and version) and `package.rs` (ids, headers,
//! stacks, changes, kits and unknown versions), `text.rs` and `table_chain.rs`
//! (the pieces those messages are written from) and `tests.rs`.

mod control;
mod element;
mod occurrence;
mod package;
mod rules;
mod spec;
mod table;
mod table_chain;
mod text;

#[cfg(test)]
mod tests;
