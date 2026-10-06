//! Writing a spec's tables back into a file.
//!
//! The folder holds: `plan.rs` (the write plan: where each loop's instances
//! and each element's value come from), `build.rs` (its construction from a
//! spec, inverting every table column), `segments.rs` (the segments the
//! columns write, by repeat), `required.rs` (what a written loop requires and
//! the codes that fill it), `refusal.rs` (why a spec cannot be
//! written) and `tests.rs`.

mod build;
mod plan;
mod refusal;
mod required;
mod segments;
#[cfg(test)]
mod tests;

pub use plan::{
    ElementPlan, Instances, LoopPlan, SegmentPlan, SegmentSource, ValueSource, WritePlan,
};
pub use refusal::{PlanError, Refusal};
