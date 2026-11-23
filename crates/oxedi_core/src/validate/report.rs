//! The answer of [`validate`](super::validate) and the text it prints.

use std::fmt;

use crate::message::Counted;
use crate::spec_load::{Change, LoadedSpec, PackageId, Role, SpecError};
use crate::write::PlanError;

/// Whether a stack of packages loads.
#[derive(Debug)]
pub enum Validation {
    /// The stack loads, so it can read: what it holds.
    Yes(Box<Valid>),
    /// The stack does not load: why.
    No(Box<Invalid>),
}

/// A stack that loads.
#[derive(Debug)]
pub struct Valid {
    /// The packages the top builds on, bases first.
    pub bases: Vec<Member>,
    /// The top package.
    pub top: Member,
    /// Whether the spec's tables can be written back into a file, or why not.
    pub write: Capability<CannotWrite>,
    /// Whether the spec can mask patient data, or why not.
    pub mask: Capability<NoPatientData>,
    /// What the editions and patches of the stack changed, in stack order.
    pub changes: Vec<Change>,
    /// The composed model and its compiled spec.
    pub loaded: LoadedSpec,
}

/// What a stack that loads can do beyond reading, or why it cannot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Capability<R> {
    /// It can.
    Can,
    /// It cannot, for these reasons.
    Cannot(R),
}

/// Reasons a capability is missing, one line each.
pub trait Reasons {
    /// One line per reason: what is wrong and where, then what to do.
    fn lines(&self) -> Vec<String>;
}

impl<R: Reasons> Capability<R> {
    /// Whether the stack can.
    pub fn can(&self) -> bool {
        matches!(self, Capability::Can)
    }

    /// The reasons it cannot, one line each; empty when it can.
    pub fn reasons(&self) -> Vec<String> {
        match self {
            Capability::Can => Vec::new(),
            Capability::Cannot(reasons) => reasons.lines(),
        }
    }
}

/// Why a spec's tables cannot be written back into a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CannotWrite {
    /// The spec has no tables, so every required loop would lack a source;
    /// one reason says so instead of one refusal per loop.
    NoTables,
    /// The write plan refuses the tables.
    Refused(PlanError),
}

impl Reasons for CannotWrite {
    /// The one reason when there is no table, else each refusal and what to
    /// change to remove it.
    fn lines(&self) -> Vec<String> {
        match self {
            CannotWrite::NoTables => {
                vec!["the stack has no tables; build a views package on it".to_string()]
            }
            CannotWrite::Refused(plan) => plan
                .refusals
                .iter()
                .map(|refusal| format!("{refusal}; {}", refusal.remedy()))
                .collect(),
        }
    }
}

/// The spec flags no column as patient data, so there is nothing to mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoPatientData;

impl fmt::Display for NoPatientData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            "no column of any table is flagged \"phi\"; flag the columns that hold names, ids \
             or dates with \"phi\": \"name\", \"id\" or \"date\"",
        )
    }
}

impl Reasons for NoPatientData {
    fn lines(&self) -> Vec<String> {
        vec![self.to_string()]
    }
}

/// One package of a stack and how it relates to its bases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// The package's id.
    pub id: PackageId,
    /// How the package relates to its bases.
    pub role: Role,
}

/// A stack that does not load.
#[derive(Debug)]
pub enum Invalid {
    /// A text does not read as a package.
    Unreadable {
        /// Which text.
        text: Text,
        /// Why it does not read.
        error: SpecError,
    },
    /// The packages read, but the stack does not compose or compile.
    Unloadable {
        /// The top package's id.
        top: PackageId,
        /// Why the stack does not load.
        error: SpecError,
    },
}

/// A text given to `validate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Text {
    /// The top package's text.
    Top,
    /// A text of the pool.
    Pool {
        /// 1-based position in the pool.
        position: usize,
    },
}

impl Valid {
    /// The top package's id.
    pub fn id(&self) -> &PackageId {
        &self.top.id
    }

    /// The spec's name.
    pub fn name(&self) -> &str {
        self.loaded.spec.name()
    }

    /// Every package of the stack, bases first, the top last.
    pub fn members(&self) -> impl Iterator<Item = &Member> {
        self.bases.iter().chain(std::iter::once(&self.top))
    }

    /// One line per change, in stack order: the change and the package that
    /// made it.
    pub fn change_lines(&self) -> Vec<String> {
        self.changes
            .iter()
            .map(|change| format!("{change} (package \"{}\")", change.package))
            .collect()
    }
}

// The members as the first lines show them: `"id" (role)`, comma separated.
struct Members<'a>(&'a Valid);

impl fmt::Display for Members<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, member) in self.0.members().enumerate() {
            let separator = if index == 0 { "" } else { ", " };
            write!(f, "{separator}\"{}\" ({})", member.id, member.role.word())?;
        }
        Ok(())
    }
}

// The changes line: `changes: none`, or the count and one numbered line each.
struct Changes<'a>(&'a Valid);

impl fmt::Display for Changes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lines = self.0.change_lines();
        if lines.is_empty() {
            return f.write_str("changes: none");
        }
        write!(f, "changes ({}):", lines.len())?;
        for (index, line) in lines.iter().enumerate() {
            write!(f, "\n{}. {line}", index + 1)?;
        }
        Ok(())
    }
}

// A capability's line: `can <what>`, or `cannot <what> (n reasons):` and one
// numbered line per reason.
struct Line<'a, R>(&'static str, &'a Capability<R>);

impl<R: Reasons> fmt::Display for Line<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reasons = self.1.reasons();
        if self.1.can() {
            return write!(f, "can {}", self.0);
        }
        write!(
            f,
            "cannot {} ({}):",
            self.0,
            Counted(reasons.len(), "reason", "reasons")
        )?;
        for (index, line) in reasons.iter().enumerate() {
            write!(f, "\n{}. {line}", index + 1)?;
        }
        Ok(())
    }
}

impl fmt::Display for Valid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "yes: stack \"{}\" loads as spec \"{}\"\npackages, bases first: {}\ncan read\n{}\n{}\n{}",
            self.id(),
            self.name(),
            Members(self),
            Line("write", &self.write),
            Line("mask", &self.mask),
            Changes(self)
        )
    }
}

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Invalid::Unreadable {
                text: Text::Top,
                error,
            } => write!(f, "no: the top package does not read: {error}"),
            Invalid::Unreadable {
                text: Text::Pool { position },
                error,
            } => write!(
                f,
                "no: package {position} of the pool does not read: {error}"
            ),
            Invalid::Unloadable { top, error } => {
                write!(f, "no: stack \"{top}\" does not load: {error}")
            }
        }
    }
}

impl fmt::Display for Validation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Validation::Yes(valid) => write!(f, "{valid}"),
            Validation::No(invalid) => write!(f, "{invalid}"),
        }
    }
}
