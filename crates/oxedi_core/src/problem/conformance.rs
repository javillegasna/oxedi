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

impl fmt::Display for WrongStack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the kit checks package \"{}\", but the stack's top package is \"{}\"; run the kit \
             on the stack that has \"{}\" on top, or change the kit's \"package\" to \"{}\"",
            self.kit, self.stack, self.kit, self.stack
        )
    }
}
