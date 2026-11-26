impl fmt::Display for BalanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BalanceError::EmptyName => write!(f, "the rule name is empty"),
            BalanceError::UnknownLoop { key, name } => {
                write!(
                    f,
                    "{key} names loop {name:?}, which the spec does not declare"
                )
            }
            BalanceError::NoTerms { key } => {
                write!(f, "{key} is empty; list at least one value")
            }
            BalanceError::LoopOutsidePer {
                key,
                loop_name,
                per,
            } => write!(
                f,
                "{key} reads loop {loop_name:?}, which is neither the rule's \"per\" loop \
                 {per:?} nor a loop below it"
            ),
            BalanceError::UnknownOccurrence {
                key,
                loop_name,
                occurrence,
            } => write!(
                f,
                "{key} names occurrence {occurrence:?}, which loop {loop_name:?} does not declare"
            ),
            BalanceError::NoElements { key } => {
                write!(f, "{key} is empty; list at least one element position")
            }
            BalanceError::UndefinedElement { key, place } => write!(
                f,
                "{key} names {place}, which the \"segments\" section does not define"
            ),
            BalanceError::NotDecimal { key, place, found } => write!(
                f,
                "{key} names {place}, of type {found}; a balancing value must be of type R"
            ),
            BalanceError::RepeatedValue { key, first, place } => write!(
                f,
                "{key} counts {place} of the same occurrence that {first} already counts"
            ),
        }
    }
}

/// The operations a summary may hold, as the message lists them.
const OPERATIONS: &str = "count, distinct, sum, row";

impl fmt::Display for SummaryDefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SummaryDefError::EmptyName => write!(f, "the name is empty"),
            SummaryDefError::Operations { found } => write!(f, "{}", Held(found)),
            SummaryDefError::UnknownTable { key, table, known } => write!(
                f,
                "{key} names the table {table:?}, which the spec does not define; \
                 its tables are: {}",
                known.join(", ")
            ),
            SummaryDefError::UnknownColumn {
                key,
                table,
                column,
                known,
            } => write!(
                f,
                "{key} names the column {column:?}, which the table {table:?} does not \
                 declare; its columns are: {}",
                known.join(", ")
            ),
            SummaryDefError::NotDecimal {
                key,
                table,
                column,
                found,
            } => write!(
                f,
                "{key} names the column {column:?} of the table {table:?}, which reads \
                 {found}; sum needs a decimal (R) column"
            ),
            SummaryDefError::NoFields { key } => write!(f, "{key} lists no field"),
            SummaryDefError::RepeatedField { key, name, first } => {
                write!(f, "{key} repeats the field {name:?} first named at {first}")
            }
        }
    }
}

/// The operations a summary holds, in words.
struct Held<'a>(&'a [String]);

impl fmt::Display for Held<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            [] => write!(f, "holds no operation"),
            found => write!(f, "holds {} operations ({})", found.len(), found.join(", ")),
        }?;
        write!(f, "; a summary holds exactly one of {OPERATIONS}")
    }
}
