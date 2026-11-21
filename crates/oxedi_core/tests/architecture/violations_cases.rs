//! Cases of the checks, over small in-memory inputs.

use super::scan::{Target, Use};
use super::table::{Exception, Row, row};
use super::violations::{Violation, violations};
use std::collections::BTreeSet;

fn to(name: &str) -> Target {
    Target::Module(name.to_owned())
}

/// A use in `file`, whose first folder is the module it belongs to.
fn at(file: &str, line: usize, target: Target, path: &str) -> Use {
    let from = file.split('/').next().unwrap_or_default().to_owned();
    Use {
        from,
        to: target,
        file: file.to_owned(),
        line,
        path: path.to_owned(),
    }
}

const A_USES_B: &[Row] = &[row("a", &["b"]), row("b", &[])];
const APART: &[Row] = &[row("a", &[]), row("b", &[])];
const A_B_A: &[Row] = &[row("a", &["b"]), row("b", &["a"])];
const NONE: &[Exception] = &[];
const A_TO_B: &[Exception] = &[Exception {
    from: "a",
    to: "b",
    reason: "a test",
}];

type Case = (
    &'static str,
    Vec<Use>,
    &'static [Row],
    &'static [Exception],
    Vec<Violation>,
);

/// The core has the modules `a` and `b`, the names of the rows of every case's table.
fn cases() -> Vec<Case> {
    let a_b = || at("a/mod.rs", 1, to("b"), "crate::b::X");
    let b_a = || at("b/mod.rs", 1, to("a"), "crate::a::Y");
    let root = || at("a/mod.rs", 4, Target::Root("X".into()), "crate::X");
    let outside = || at("a/mod.rs", 9, Target::Outside, "super::b::X");
    let (a, b): (String, String) = ("a".into(), "b".into());
    vec![
        (
            "an unlisted use",
            vec![a_b()],
            APART,
            NONE,
            vec![Violation::NotAllowed(a_b())],
        ),
        ("a listed use", vec![a_b()], A_USES_B, NONE, vec![]),
        ("an excepted use", vec![a_b()], APART, A_TO_B, vec![]),
        (
            "a row entry no file uses",
            vec![],
            A_USES_B,
            NONE,
            vec![Violation::Unused {
                from: a.clone(),
                to: b.clone(),
            }],
        ),
        (
            "an exception no file uses",
            vec![],
            APART,
            A_TO_B,
            vec![Violation::UnusedException {
                from: a.clone(),
                to: b.clone(),
            }],
        ),
        (
            "an exception the row lists",
            vec![a_b()],
            A_USES_B,
            A_TO_B,
            vec![Violation::RedundantException { from: a, to: b }],
        ),
        (
            "a cycle in the rows",
            vec![a_b(), b_a()],
            A_B_A,
            NONE,
            vec![Violation::Cycle {
                path: vec!["a".into(), "b".into(), "a".into()],
            }],
        ),
        (
            "a path through the crate root",
            vec![root()],
            APART,
            NONE,
            vec![Violation::RootPath(root())],
        ),
        (
            "a super leaving the module",
            vec![outside()],
            APART,
            NONE,
            vec![Violation::LeavesModule(outside())],
        ),
    ]
}

fn modules(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|m| (*m).to_owned()).collect()
}

#[test]
fn each_violation_is_reported() {
    for (name, uses, table, exceptions, expected) in cases() {
        let got = violations(&modules(&["a", "b"]), &uses, table, exceptions);
        assert_eq!(got, expected, "case {name}");
        assert!(
            exceptions.iter().all(|e| !e.reason.is_empty()),
            "case {name}"
        );
    }
}

#[test]
fn a_module_and_a_row_must_match() {
    let no_row = violations(&modules(&["a", "b", "c"]), &[], APART, NONE);
    assert_eq!(no_row, [Violation::NoRow { module: "c".into() }]);
    let no_module = violations(&modules(&["a"]), &[], APART, NONE);
    assert_eq!(no_module, [Violation::NoModule { module: "b".into() }]);
}

#[test]
fn each_violation_explains_itself() {
    let cases = [
        (
            Violation::NotAllowed(at(
                "check/envelope.rs",
                12,
                to("project"),
                "crate::project::Projector",
            )),
            "check uses project, which the table does not allow · check/envelope.rs:12 · crate::project::Projector · remove the use, or add \"project\" to the row of check in tests/architecture/table.rs when the design allows it",
        ),
        (
            Violation::RootPath(at(
                "check/mod.rs",
                4,
                Target::Root("Segment".into()),
                "crate::Segment",
            )),
            "check reaches Segment through the crate root · check/mod.rs:4 · crate::Segment · name its module in the path (crate::<module>::Segment), so the dependency can be read",
        ),
        (
            Violation::RootPath(at("check/mod.rs", 2, Target::Root("*".into()), "crate::*")),
            "check reaches * through the crate root · check/mod.rs:2 · crate::* · name its module in the path (crate::<module>::*), so the dependency can be read",
        ),
        (
            Violation::LeavesModule(at("engine/mod.rs", 9, Target::Outside, "super::spec::Spec")),
            "engine leaves its module through super · engine/mod.rs:9 · super::spec::Spec · write the path from the crate (crate::<module>::...), so the dependency can be read",
        ),
        (
            Violation::Unused {
                from: "check".into(),
                to: "trivia".into(),
            },
            "the table lets check use trivia, and no file of check does · remove \"trivia\" from the row of check in tests/architecture/table.rs",
        ),
        (
            Violation::UnusedException {
                from: "delimiters".into(),
                to: "segment".into(),
            },
            "the exception delimiters -> segment is no longer needed: no file of delimiters uses segment · remove it from EXCEPTIONS in tests/architecture/table.rs",
        ),
        (
            Violation::RedundantException {
                from: "check".into(),
                to: "trivia".into(),
            },
            "the exception check -> trivia is no longer needed: the row of check already lists trivia · remove it from EXCEPTIONS in tests/architecture/table.rs",
        ),
        (
            Violation::Cycle {
                path: vec!["a".into(), "b".into(), "a".into()],
            },
            "the table allows a cycle: a -> b -> a · remove one of these dependencies from the rows of tests/architecture/table.rs",
        ),
        (
            Violation::NoRow {
                module: "walk".into(),
            },
            "the module walk has no row in the table · add a row for walk to tests/architecture/table.rs with the modules it may use",
        ),
        (
            Violation::NoModule {
                module: "walk".into(),
            },
            "the table has a row for walk, and src has no such module · remove the row from tests/architecture/table.rs",
        ),
    ];
    for (violation, text) in cases {
        assert_eq!(violation.to_string(), text);
    }
}
