//! Running a conformance kit against a spec and comparing what it gives with
//! what the kit expects.
//!
//! `diff` the first line where two texts differ; `tests` its tests. The module
//! is for tools that check specs: the browser module never names it.

mod diff;
#[cfg(test)]
mod tests;

pub use diff::{Line, LineDifference, first_difference};
