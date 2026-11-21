//! Compares what the files use with what the table allows, and says every difference.

use super::scan::{Target, Use};
use super::table::{Exception, Row};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum Violation {
    NotAllowed(Use),
    RootPath(Use),
    LeavesModule(Use),
    Unused { from: String, to: String },
    UnusedException { from: String, to: String },
    RedundantException { from: String, to: String },
    NoRow { module: String },
    NoModule { module: String },
    Cycle { path: Vec<String> },
}

const TABLE_FILE: &str = "tests/architecture/table.rs";

/// The name a use mentions: its module, or the root item it reaches.
impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Module(name) | Self::Root(name) => f.write_str(name),
            Self::Outside => f.write_str("super"),
        }
    }
}

/// Where a use is and how it is written.
impl fmt::Display for Use {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{} · {}", self.file, self.line, self.path)
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAllowed(u @ Use { from, to, .. }) => write!(
                f,
                "{from} uses {to}, which the table does not allow · {u} · remove the use, or add \"{to}\" to the row of {from} in {TABLE_FILE} when the design allows it"
            ),
            Self::RootPath(u @ Use { from, to, .. }) => write!(
                f,
                "{from} reaches {to} through the crate root · {u} · name its module in the path (crate::<module>::{to}), so the dependency can be read"
            ),
            Self::LeavesModule(u) => write!(
                f,
                "{} leaves its module through super · {u} · write the path from the crate (crate::<module>::...), so the dependency can be read",
                u.from
            ),
            Self::Unused { from, to } => write!(
                f,
                "the table lets {from} use {to}, and no file of {from} does · remove \"{to}\" from the row of {from} in {TABLE_FILE}"
            ),
            Self::UnusedException { from, to } => write!(
                f,
                "the exception {from} -> {to} is no longer needed: no file of {from} uses {to} · remove it from EXCEPTIONS in {TABLE_FILE}"
            ),
            Self::RedundantException { from, to } => write!(
                f,
                "the exception {from} -> {to} is no longer needed: the row of {from} already lists {to} · remove it from EXCEPTIONS in {TABLE_FILE}"
            ),
            Self::NoRow { module } => write!(
                f,
                "the module {module} has no row in the table · add a row for {module} to {TABLE_FILE} with the modules it may use"
            ),
            Self::NoModule { module } => write!(
                f,
                "the table has a row for {module}, and src has no such module · remove the row from {TABLE_FILE}"
            ),
            Self::Cycle { path } => write!(
                f,
                "the table allows a cycle: {} · remove one of these dependencies from the rows of {TABLE_FILE}",
                path.join(" -> ")
            ),
        }
    }
}

type Edge<'a> = (&'a str, &'a str);

fn violation_of(kind: fn(String, String) -> Violation, (from, to): Edge) -> Violation {
    kind(from.to_owned(), to.to_owned())
}

/// Everything wrong between what the files use and what the table says.
pub fn violations(
    modules: &BTreeSet<String>,
    uses: &[Use],
    table: &[Row],
    exceptions: &[Exception],
) -> Vec<Violation> {
    let used: BTreeSet<Edge> = uses
        .iter()
        .filter_map(|u| match &u.to {
            Target::Module(to) => Some((u.from.as_str(), to.as_str())),
            _ => None,
        })
        .collect();
    let listed: BTreeSet<Edge> = table
        .iter()
        .flat_map(|r| r.uses.iter().map(|&to| (r.module, to)))
        .collect();
    let excepted: BTreeSet<Edge> = exceptions.iter().map(|e| (e.from, e.to)).collect();
    let allowed: BTreeSet<Edge> = listed.union(&excepted).copied().collect();

    let unused = |from, to| Violation::Unused { from, to };
    let unused_exception = |from, to| Violation::UnusedException { from, to };
    let redundant = |from, to| Violation::RedundantException { from, to };
    let mut found: Vec<Violation> = uses
        .iter()
        .filter_map(|u| use_violation(u, &allowed))
        .collect();
    found.extend(listed.difference(&used).map(|&e| violation_of(unused, e)));
    found.extend(
        excepted
            .difference(&used)
            .map(|&e| violation_of(unused_exception, e)),
    );
    found.extend(
        excepted
            .intersection(&listed)
            .map(|&e| violation_of(redundant, e)),
    );
    found.extend(missing_modules(modules, table));
    found.extend(find_cycle(table).map(|path| Violation::Cycle { path }));
    found
}

/// What is wrong with one use, if anything.
fn use_violation(u: &Use, allowed: &BTreeSet<Edge>) -> Option<Violation> {
    match &u.to {
        Target::Module(to) if allowed.contains(&(u.from.as_str(), to.as_str())) => None,
        Target::Module(_) => Some(Violation::NotAllowed(u.clone())),
        Target::Root(_) => Some(Violation::RootPath(u.clone())),
        Target::Outside => Some(Violation::LeavesModule(u.clone())),
    }
}

/// The modules without a row and the rows without a module.
fn missing_modules(modules: &BTreeSet<String>, table: &[Row]) -> Vec<Violation> {
    let rows: BTreeSet<&str> = table.iter().map(|r| r.module).collect();
    let no_row = modules.iter().filter(|m| !rows.contains(m.as_str()));
    let no_module = rows.iter().filter(|r| !modules.contains(**r));
    no_row
        .map(|m| Violation::NoRow { module: m.clone() })
        .chain(no_module.map(|m| Violation::NoModule {
            module: (*m).to_owned(),
        }))
        .collect()
}

#[derive(Clone, Copy)]
enum Colour {
    /// On the walk's current path.
    Grey,
    /// Fully explored, no cycle through it.
    Black,
}

/// One cycle of the rows, as module names in order with the first repeated at the end.
fn find_cycle(table: &[Row]) -> Option<Vec<String>> {
    let graph: BTreeMap<&str, &[&str]> = table.iter().map(|r| (r.module, r.uses)).collect();
    let mut colour = BTreeMap::new();
    let mut stack = Vec::new();
    graph
        .keys()
        .find_map(|&node| walk(node, &graph, &mut colour, &mut stack))
}

/// Depth-first walk with the three colours: white (absent from `colour`), grey and black.
fn walk<'a>(
    node: &'a str,
    graph: &BTreeMap<&'a str, &'a [&'a str]>,
    colour: &mut BTreeMap<&'a str, Colour>,
    stack: &mut Vec<&'a str>,
) -> Option<Vec<String>> {
    match colour.get(node) {
        Some(Colour::Black) => return None,
        Some(Colour::Grey) => return Some(cycle_from(node, stack)),
        None => {}
    }
    colour.insert(node, Colour::Grey);
    stack.push(node);
    let next = graph.get(node).copied().unwrap_or_default();
    let found = next.iter().find_map(|&n| walk(n, graph, colour, stack));
    stack.pop();
    colour.insert(node, Colour::Black);
    found
}

/// The part of the walk's path from `node` on, closed with `node`.
fn cycle_from(node: &str, stack: &[&str]) -> Vec<String> {
    let start = stack.iter().position(|&s| s == node).unwrap_or_default();
    let closed = stack[start..].iter().chain(std::iter::once(&node));
    closed.map(|s| (*s).to_owned()).collect()
}
