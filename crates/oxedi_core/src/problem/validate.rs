//! The text of what a validation says about a stack of packages.

use std::fmt;

use crate::message::Counted;
use crate::validate::{
    CannotWrite, Capability, Invalid, NoPatientData, Reasons, Text, Valid, Validation,
};

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

impl Valid {
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
