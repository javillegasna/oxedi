//! The answer of [`validate`](super::validate) and the text it prints.

use std::fmt;

use crate::spec_load::{Change, LoadedSpec, PackageId, Role, SpecError};

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
    /// What the editions and patches of the stack changed, in stack order.
    pub changes: Vec<Change>,
    /// The composed model and its compiled spec.
    pub loaded: LoadedSpec,
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

impl fmt::Display for Valid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "yes: stack \"{}\" loads as spec \"{}\"\npackages, bases first: {}\ncan read\n{}",
            self.id(),
            self.name(),
            Members(self),
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
