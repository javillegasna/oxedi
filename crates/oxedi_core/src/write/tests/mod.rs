//! Tests of the write plan (`plan.rs`), of the text of the writer's
//! findings, origins and errors (`display.rs`), of the origin a
//! read-back diagnostic gets (`origin.rs`) and of a cell's element text
//! (`render.rs`), of where the plan writes each column against where the
//! projector reads it (`places.rs`) and of what each refusal says to do
//! (`remedy.rs`). The writer itself is tested on parsed files in the crate's `tests/write_*.rs`.

use crate::message::ElementRef;
use crate::spec::{Selector, Trigger};

mod display;
mod envelope;
mod layout;
mod origin;
mod places;
mod plan;
mod remedy;
mod render;

/// The selector of a trigger with no conditions, for the refusals that name one.
fn bare(segment: &[u8]) -> Selector {
    Selector::Trigger(Trigger {
        segment: segment.to_vec(),
        conditions: Vec::new(),
    })
}

/// A simple element reference such as `CLP01`.
fn at(segment: &[u8], element: usize) -> ElementRef {
    ElementRef::simple(segment.to_vec(), element)
}
