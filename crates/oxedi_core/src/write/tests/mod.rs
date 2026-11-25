//! Tests of the write plan (`plan.rs`), of the text of the writer's
//! findings, origins and errors (`display.rs`), of the origin a
//! read-back diagnostic gets (`origin.rs`) and of a cell's element text
//! (`render.rs`), of where the plan writes each column against where the
//! projector reads it (`places.rs`), of the writer's message pieces
//! (`wording.rs`) and of what each refusal says to do (`remedy.rs`). The writer itself is tested on parsed files in the crate's `tests/write_*.rs`.

mod display;
mod envelope;
mod layout;
mod origin;
mod places;
mod plan;
mod remedy;
mod render;
mod wording;
