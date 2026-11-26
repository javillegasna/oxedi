//! Running a conformance kit against a spec and comparing what it gives with
//! what the kit expects.
//!
//! `diff` the first line where two texts differ; `run` the runner over bytes;
//! `render` the text of an output, which the runner compares and a golden
//! holds; `error` why a kit does not run on a stack; `report` what it reports
//! (its text is in `problem/conformance.rs`); `tests` their tests. The module
//! is for tools that check specs: the browser module never names it.

mod diff;
mod error;
mod render;
mod report;
mod run;
#[cfg(test)]
mod tests;

pub use diff::{Line, LineDifference, first_difference};
pub use error::WrongStack;
pub use render::render;
pub use report::{CaseFault, CaseReport, Difference, KitReport, Outcome};
pub use run::run_kit;
