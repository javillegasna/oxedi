//! The only place that writes message text.
//!
//! The layers raise problems as data (the rule that failed, where, the
//! offending value); this module writes their `Display` and remedies, so a
//! wording changes in one folder and no layer holds prose.
//!
//! The folder holds: `diagnostic/` (the text of a diagnostic and its rules),
//! `write/` (the text of the writer's refusals, findings and errors) and
//! `load/` (the text of the problems a spec, a package or a kit raises).

mod diagnostic;
mod load;
mod write;
