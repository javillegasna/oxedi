//! Cases of the reader: what it finds in a small piece of source.

use super::scan::{Target, Use, uses_in};
use std::collections::BTreeSet;
use std::path::Path;

fn module(name: &str) -> Target {
    Target::Module(name.to_owned())
}

fn root(name: &str) -> Target {
    Target::Root(name.to_owned())
}

/// The use the reader must report for a file at `rel`.
fn found(rel: &str, to: Target, line: usize, path: &str) -> Use {
    Use {
        from: rel.split('/').next().unwrap_or_default().to_owned(),
        to,
        file: rel.to_owned(),
        line,
        path: path.to_owned(),
    }
}

fn cases() -> Vec<(&'static str, &'static str, &'static str, Vec<Use>)> {
    vec![
        (
            "a_plain_use",
            "check/mod.rs",
            "use crate::spec::Spec;",
            vec![found(
                "check/mod.rs",
                module("spec"),
                1,
                "crate::spec::Spec",
            )],
        ),
        (
            "a_path_in_an_expression",
            "write/plan.rs",
            "fn f() {\n    let x = 1;\n    crate::value::parse_dt(x)\n}",
            vec![found(
                "write/plan.rs",
                module("value"),
                3,
                "crate::value::parse_dt",
            )],
        ),
        (
            "a_grouped_use_over_lines",
            "check/mod.rs",
            "use crate::{\n    segment::Segment,\n    spec::{Spec, LoopId},\n};",
            vec![
                found(
                    "check/mod.rs",
                    module("segment"),
                    2,
                    "crate::segment::Segment",
                ),
                found("check/mod.rs", module("spec"), 3, "crate::spec"),
            ],
        ),
        (
            "its_own_module",
            "spec/tables.rs",
            "use crate::spec::Spec;",
            vec![],
        ),
        (
            "a_line_comment",
            "check/mod.rs",
            "// see crate::spec::Spec\nfn f() {}",
            vec![],
        ),
        (
            "a_doc_link",
            "check/mod.rs",
            "/// [`X`]: crate::Document::segment",
            vec![],
        ),
        (
            "a_block_comment",
            "check/mod.rs",
            "/* crate::spec::Spec\n  crate::value::V */",
            vec![],
        ),
        (
            "code_after_a_block_comment_keeps_its_line",
            "check/mod.rs",
            "/* one\ntwo */ use crate::spec::Spec;",
            vec![found(
                "check/mod.rs",
                module("spec"),
                2,
                "crate::spec::Spec",
            )],
        ),
        (
            "a_string_literal",
            "write/plan.rs",
            "let p = \"crate::spec::model::is_default\";",
            vec![found(
                "write/plan.rs",
                module("spec"),
                1,
                "crate::spec::model::is_default",
            )],
        ),
        (
            "a_comment_marker_in_a_string",
            "write/plan.rs",
            "let a = \"//\"; use crate::spec::Spec;",
            vec![found(
                "write/plan.rs",
                module("spec"),
                1,
                "crate::spec::Spec",
            )],
        ),
        (
            "a_macro_path",
            "spec/model/declare.rs",
            "($crate::value::parse)",
            vec![found(
                "spec/model/declare.rs",
                module("value"),
                1,
                "crate::value::parse",
            )],
        ),
        (
            "a_root_path",
            "check/mod.rs",
            "use crate::Segment;",
            vec![found("check/mod.rs", root("Segment"), 1, "crate::Segment")],
        ),
        (
            "a_glob_from_the_crate_root",
            "check/mod.rs",
            "\nuse crate::*;",
            vec![found("check/mod.rs", root("*"), 2, "crate::*")],
        ),
        (
            "a_glob_inside_a_group",
            "check/mod.rs",
            "use crate::{\n    *,\n    spec::Spec,\n};",
            vec![
                found("check/mod.rs", root("*"), 2, "crate::*"),
                found("check/mod.rs", module("spec"), 3, "crate::spec::Spec"),
            ],
        ),
        (
            "super_leaving_the_module",
            "engine/mod.rs",
            "use super::spec::Spec;",
            vec![found(
                "engine/mod.rs",
                Target::Outside,
                1,
                "super::spec::Spec",
            )],
        ),
        (
            "super_inside_the_module",
            "column/table.rs",
            "use super::Cell;",
            vec![],
        ),
        (
            "two_supers_leaving_a_nested_file",
            "column/table.rs",
            "use super::super::Cell;",
            vec![found(
                "column/table.rs",
                Target::Outside,
                1,
                "super::super::Cell",
            )],
        ),
        (
            "a_name_that_ends_in_crate",
            "check/mod.rs",
            "my_crate::spec::Spec",
            vec![],
        ),
    ]
}

#[test]
fn the_reader_finds_each_dependency_in_its_place() {
    let modules: BTreeSet<String> = ["check", "engine", "segment", "spec", "value", "write"]
        .map(str::to_owned)
        .into();
    for (name, rel, text, expected) in cases() {
        let got = uses_in(Path::new(rel), text, &modules);
        assert_eq!(got, expected, "case {name}");
    }
}
