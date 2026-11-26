//! The one shape every problem family has: what kind of problem it is,
//! where it points and what extra a family carries, and a record that holds
//! all of it with the text.
//!
//! The folder holds: this file (the trait, the place, the detail and the
//! record), and one file per family that implements the trait.

use std::fmt;

use crate::diagnostic::{LoopRef, Rule, SnipLevel};
use crate::spec_load::PackageId;
use crate::write::{EnvelopeSource, ValueOrigin};

mod diagnostic;
mod finding;
#[cfg(test)]
mod tests;

/// A problem a layer raises: it has a text (`Display`), a kind, a place and,
/// for some families, a detail.
pub trait Problem: fmt::Display {
    /// The variant's name as written in Rust, e.g. `UnknownSegment`. It is a
    /// stable word: callers may match on it, so renaming a variant is a break.
    fn kind(&self) -> &'static str;

    /// The most specific place the problem's fields name.
    fn place(&self) -> Place<'_>;

    /// What the family carries beside the place.
    fn detail(&self) -> Detail<'_> {
        Detail::Plain
    }

    /// The kind, place, text and detail together.
    fn record(&self) -> ProblemRecord<'_> {
        ProblemRecord {
            kind: self.kind(),
            place: self.place(),
            text: self.to_string(),
            detail: self.detail(),
        }
    }
}

/// Where a problem points. Every place borrows from the problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place<'a> {
    /// A position in a file.
    File {
        /// 0-based segment index; `None` at the end of the stream.
        segment: Option<usize>,
        /// 1-based element position, when the problem is about one.
        element: Option<usize>,
        /// 1-based component position, when the problem is about one.
        component: Option<usize>,
        /// The open loops, outermost first.
        path: &'a [LoopRef],
    },
    /// An entry of a spec.
    Spec(SpecPlace<'a>),
    /// A cell, a row or a whole table of the tables.
    Cell {
        /// The table.
        table: &'a str,
        /// 0-based row; none for a whole table.
        row: Option<usize>,
        /// The column; none for a whole table or row.
        column: Option<&'a str>,
    },
    /// A field of the envelope.
    Envelope {
        /// The field; `name()` writes it, e.g. `sender_id`.
        field: EnvelopeSource,
    },
    /// A package of a stack.
    Package {
        /// The package's id.
        id: &'a PackageId,
    },
    /// A conformance kit.
    Kit {
        /// The case, when the problem is about one.
        case: Option<&'a str>,
        /// The file, when the problem is about one.
        file: Option<&'a str>,
    },
    /// The problem names no place.
    Nowhere,
}

/// An entry of a spec a problem points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecPlace<'a> {
    /// The spec as a whole.
    Whole,
    /// A key path from the spec's root.
    Path(&'a str),
    /// A loop, and one of its occurrences when the problem is about one.
    Loop {
        /// The loop's name.
        name: &'a str,
        /// The occurrence's name.
        occurrence: Option<&'a str>,
    },
    /// A table, and one of its columns when the problem is about one.
    Table {
        /// The table's name.
        name: &'a str,
        /// The column's name.
        column: Option<&'a str>,
    },
    /// A segment definition, and one of its elements when the problem is
    /// about one.
    Segment {
        /// The segment id.
        id: &'a str,
        /// The element's position as the spec writes it.
        position: Option<&'a str>,
    },
    /// A balancing rule.
    Balance {
        /// The rule's name.
        rule: &'a str,
    },
    /// A summary.
    Summary {
        /// The summary's name.
        name: &'a str,
    },
}

/// What a family carries beside the kind and the place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detail<'a> {
    /// A reading diagnostic.
    Diagnostic {
        /// The SNIP level.
        level: SnipLevel,
        /// The rule that failed.
        rule: &'a Rule,
        /// The offending value as it appears in the file.
        datum: &'a [u8],
        /// Who reported it.
        origin: &'a str,
        /// The external validator's own code, when it has one.
        code: Option<&'a str>,
    },
    /// A write finding.
    Finding {
        /// Where the value comes from, when the finding is about a value.
        origin: Option<&'a ValueOrigin>,
    },
    /// Nothing more.
    Plain,
}

/// A problem as data: its kind, place, text and detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProblemRecord<'a> {
    /// The variant's name.
    pub kind: &'static str,
    /// Where it points.
    pub place: Place<'a>,
    /// Its text.
    pub text: String,
    /// What its family carries.
    pub detail: Detail<'a>,
}
