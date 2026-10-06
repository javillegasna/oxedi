//! Writing a spec's tables back into a file.
//!
//! The folder holds: `plan.rs` (the write plan: where each loop's instances
//! and each element's value come from), `build.rs` (its construction from a
//! spec, inverting every table column), `place.rs` (the loops written
//! around the written ones and the position order of segments), `codes.rs`
//! (the occurrence a column's segment and `where` select, and the refusal of
//! fixed codes its code lists do not allow), `segments.rs` (the segments the
//! columns write, by repeat), `required.rs` (what a written loop requires and
//! the codes that fill it), `refusal.rs` (why a spec cannot be written);
//! `writer.rs` (the entry points [`write()`] and [`write_with_findings`]),
//! `envelope.rs` (the caller's [`Envelope`] and the envelope segments'
//! values), `data.rs` (the caller's tables bound to the spec's), `nest.rs`
//! (the rows nested by their references), `layout.rs` (what the walk works
//! out once per spec), `emit.rs` (the walk that writes the file),
//! `walk.rs` (the instances a loop takes inside an instance), `cells.rs`
//! (the parts of the segment being written),
//! `render.rs` (values as text and segments as bytes), `trace.rs` (where
//! each written element came from), `finding.rs` (the findings and the write
//! error) and `tests/`.

mod build;
mod cells;
mod codes;
mod data;
mod emit;
mod envelope;
mod finding;
mod layout;
mod nest;
mod place;
mod plan;
mod refusal;
mod render;
mod required;
mod segments;
#[cfg(test)]
mod tests;
mod trace;
mod walk;
mod writer;

pub use envelope::Envelope;
pub use finding::{Finding, Origin, WriteError};
pub use plan::{
    ElementPlan, Instances, LoopPlan, SegmentPlan, SegmentSource, Unwritten, ValueSource, WritePlan,
};
pub use refusal::{PlanError, Refusal};
pub use writer::{write, write_with_findings};
