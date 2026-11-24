//! Running a conformance kit against a spec and comparing what it gives with
//! what the kit expects.
//!
//! `diff` the first line where two texts differ; `render` the text of an
//! output, which a kit's expected file and a golden hold; `tests` their tests.
//! The module is for tools that check specs: the browser module never names it.

mod diff;
mod render;
#[cfg(test)]
mod tests;

pub use diff::{Line, LineDifference, first_difference};
pub use render::render;
