//! What running a kit reports, per case, and the text it prints.

use std::fmt;

use super::diff::{Line, LineDifference};
use crate::delimiters::ISA_ID;
use crate::document::DocumentError;
use crate::spec_load::{KitOutput, PackageId};
use crate::vocabulary::Vocabulary;

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

impl fmt::Display for KitReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let passing = self
            .cases
            .iter()
            .filter(|case| case.outcome == Outcome::Passed)
            .count();
        write!(
            f,
            "kit \"{}\": {passing} of {} cases pass",
            self.package,
            self.cases.len()
        )?;
        self.cases.iter().try_for_each(|case| write!(f, "\n{case}"))
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

impl fmt::Display for CaseReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "case {:?}: ", self.name)?;
        match &self.outcome {
            Outcome::Passed => write!(f, "passes"),
            Outcome::Broken(fault) => write!(f, "cannot run: {fault}"),
            Outcome::Differs(differences) => {
                let texts: Vec<String> = differences.iter().map(Difference::to_string).collect();
                write!(f, "differs: {}", texts.join("; "))
            }
        }
    }
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

impl fmt::Display for CaseFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CaseFault::MissingFile { path } => write!(
                f,
                "the kit names {path:?}, which was not given; give the file to the runner"
            ),
            CaseFault::NotText { path } => write!(
                f,
                "{path:?} is not UTF-8 text; an expected output is a text file"
            ),
            CaseFault::NotADocument { path, reason } => write!(
                f,
                "{path:?} is not an X12 document: {reason}; give a file that opens with {ISA_ID}"
            ),
        }
    }
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

impl fmt::Display for Difference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}) line {}: expected {}, found {}",
            self.output.word(),
            self.file,
            self.at.line,
            Shown(&self.at.expected),
            Shown(&self.at.actual)
        )
    }
}

/// A line as a message quotes it, or `the end`.
struct Shown<'a>(&'a Line);

impl fmt::Display for Shown<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Line::Text(text) => write!(f, "{text:?}"),
            Line::End => write!(f, "the end"),
        }
    }
}
