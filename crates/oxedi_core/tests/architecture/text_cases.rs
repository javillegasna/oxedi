//! Cases of the text rule, over small in-memory sources.

use super::blank::blank_comments_and_escapes;
use super::table::Listed;
use super::text::{Found, List, TextViolation, displays_in, renderers_in, text_violations};
use std::path::Path;

fn found(file: &str, line: usize, name: &str) -> Found {
    Found {
        file: file.to_owned(),
        line,
        name: name.to_owned(),
    }
}

fn displays(rel: &str, source: &str) -> Vec<Found> {
    displays_in(Path::new(rel), &blank_comments_and_escapes(source))
}

fn renderers(rel: &str, source: &str) -> Vec<Found> {
    renderers_in(Path::new(rel), &blank_comments_and_escapes(source))
}

#[test]
fn a_display_impl_outside_problem_is_found_and_explains_itself() {
    let source = "impl fmt::Display for Foo {";
    let finds = displays("check/x.rs", source);
    assert_eq!(finds, vec![found("check/x.rs", 1, "Foo")]);
    assert_eq!(
        text_violations(finds, Vec::new(), &[], &[])[0].to_string(),
        "check/x.rs:1 · impl Display for Foo outside problem/ · write a problem's text in problem/, or add Foo of check/x.rs to NOTATION in tests/architecture/table.rs with the reason"
    );
}

#[test]
fn generics_paths_and_references_are_read() {
    let source = "impl<V: AsRef<[u8]>> std::fmt::Display for Conditions<'_, V> {}\nimpl Display for Bar {}\nimpl fmt::Display for &Baz {}\nimpl<F: Fn() -> u8> fmt::Display for Qux<F> {}";
    let names: Vec<String> = displays("write/x.rs", source)
        .into_iter()
        .map(|f| f.name)
        .collect();
    assert_eq!(names, ["Conditions", "Bar", "Baz", "Qux"]);
}

#[test]
fn the_problem_module_and_test_files_are_skipped() {
    let source = "impl fmt::Display for Foo {";
    assert!(displays("problem/x.rs", source).is_empty());
    assert!(displays("check/tests.rs", source).is_empty());
    assert!(renderers("problem/x.rs", "fn render_a() {}").is_empty());
    assert!(displays("check/x.rs", "// impl fmt::Display for Foo").is_empty());
}

#[test]
fn an_unused_notation_name_is_reported() {
    let notation = [Listed {
        file: "check/x.rs",
        name: "Gone",
        reason: "a reason",
    }];
    assert_eq!(
        text_violations(Vec::new(), Vec::new(), &notation, &[]),
        vec![TextViolation::Unused(
            List::Notation,
            found("check/x.rs", 0, "Gone")
        )]
    );
}

#[test]
fn a_renderer_outside_problem_is_reported() {
    let finds = renderers("write/x.rs", "fn render_thing(a: u8) {}");
    assert_eq!(finds, vec![found("write/x.rs", 1, "render_thing")]);
    assert_eq!(
        text_violations(Vec::new(), finds, &[], &[]),
        vec![TextViolation::Outside(
            List::Renderers,
            found("write/x.rs", 1, "render_thing")
        )]
    );
}

#[test]
fn a_listed_name_is_keyed_by_its_file() {
    let notation = [Listed {
        file: "check/x.rs",
        name: "Foo",
        reason: "a reason",
    }];
    let source = "impl fmt::Display for Foo {";
    let same = displays("check/x.rs", source);
    assert!(text_violations(same, Vec::new(), &notation, &[]).is_empty());
    let other = displays("write/y.rs", source);
    assert_eq!(text_violations(other, Vec::new(), &notation, &[]).len(), 2);
}
