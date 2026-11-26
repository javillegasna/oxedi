//! What running a kit reports, per case.

use super::diff::LineDifference;
use crate::document::DocumentError;
use crate::spec_load::{KitOutput, PackageId};

/// The result of running a kit: one report per case, in name order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KitReport {
    /// The top package of the stack the kit checks.
    pub package: PackageId,
    /// Each case's report, in name order.
    pub cases: Vec<CaseReport>,
}

impl KitReport {
    /// Whether every case passed.
    pub fn passed(&self) -> bool {
        self.cases
            .iter()
            .all(|case| case.outcome == Outcome::Passed)
    }
}

/// What running one case gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseReport {
    /// The case's name.
    pub name: String,
    /// How the case ended.
    pub outcome: Outcome,
}

/// How a case ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Every expected output matches.
    Passed,
    /// The case could not run.
    Broken(CaseFault),
    /// Some expected output differs: one entry per differing output, in
    /// output order.
    Differs(Vec<Difference>),
}

/// Why a case could not run. The first fault ends the case: the files after
/// it are not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseFault {
    /// The kit names a file that was not given.
    MissingFile {
        /// The path as the kit writes it.
        path: String,
    },
    /// An expected file is not UTF-8 text.
    NotText {
        /// The path as the kit writes it.
        path: String,
    },
    /// The input is not an X12 document.
    NotADocument {
        /// The input's path as the kit writes it.
        path: String,
        /// Why the document did not read.
        reason: DocumentError,
    },
}

/// An expected output that differs from the run's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    /// The output.
    pub output: KitOutput,
    /// The file that holds the expected text, as the kit writes its path.
    pub file: String,
    /// Where the texts first differ.
    pub at: LineDifference,
}
