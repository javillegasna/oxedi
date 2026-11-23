use super::super::*;
use super::package;
use crate::spec_load::builtin::PACKAGES;
use crate::spec_load::{Builtin, SpecError};

const E: &str = "test/envelope/005010";
const D: &str = "test/dictionary/005010";
const D2: &str = "test/dictionary/004010";

// A loop and a segment `AA` with an element that bounds everything and a
// composite element with one component.
const BASE: &str = r#"{"loops":{"a":{"trigger":{"segment":"AA"},"occurrences":{"aa":{"segment":"AA","pos":0}}}},"segments":{"AA":{"elements":{"1":{"name":"code","type":"ID","min":1,"max":2,"required":true,"codes":["X","Y"]},"2":{"name":"pair","type":"AN","composite":{"1":{"name":"part","type":"ID","codes":["P","Q"]}}}}}}}"#;

fn base() -> Package {
    package(E, &["envelope"], &[], BASE)
}

fn dictionary(id: &str, bases: &[&str], spec: &str) -> Package {
    package(id, &["dictionary"], bases, spec)
}

fn element(change: &str) -> String {
    format!(r#"{{"segments":{{"AA":{{"elements":{{"1":{change}}}}}}}}}"#)
}

fn compose(top: &Package, others: &[Package]) -> Result<Composed, SpecError> {
    let mut pool = others.to_vec();
    pool.push(top.clone());
    Stack::resolve(top, &pool)?.compose_checked()
}

const TAIL: &str = "; a package only narrows its bases, unless it is an edition (it holds a layer its base holds) or a patch";

#[test]
fn widening_an_element_is_refused() {
    let cases = [
        (
            element(r#"{"codes":["X","Z"]}"#),
            r#"segments.AA.elements.1.codes: ["X","Y"] becomes ["X","Z"]"#,
        ),
        (
            element(r#"{"max":null}"#),
            "segments.AA.elements.1.max: 2 becomes no maximum",
        ),
        (
            element(r#"{"required":false}"#),
            "segments.AA.elements.1.required: required becomes not required",
        ),
        (
            element(r#"{"type":"AN"}"#),
            r#"segments.AA.elements.1.type: "ID" becomes "AN""#,
        ),
        (
            r#"{"segments":{"AA":null}}"#.to_owned(),
            "segments.AA is removed",
        ),
    ];
    for (spec, change) in cases {
        let top = dictionary(D, &[E], &spec);
        let err = compose(&top, &[base()]).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!("package \"{D}\" widens what its bases allow: {change}{TAIL}"),
            "{spec}"
        );
    }
}

#[test]
fn narrowing_an_element_is_accepted_and_not_recorded() {
    let top = dictionary(
        D,
        &[E],
        r#"{"segments":{"AA":{"elements":{"1":{"codes":["X"],"min":1,"max":1},"2":{"composite":{"1":{"codes":["P"]}}}}}}}"#,
    );
    let composed = compose(&top, &[base()]).unwrap();
    assert!(composed.changes.is_empty());
    let narrowed = &composed.model.segments["AA"].elements[&1];
    assert_eq!(narrowed.codes, Some(vec!["X".to_owned()]));
    assert_eq!(narrowed.max, Some(1));
}

#[test]
fn an_edition_records_its_changes() {
    let first = dictionary(D, &[E], r#"{"segments":{}}"#);
    let edition = dictionary(D2, &[D], &element(r#"{"codes":["X","Z"],"max":1}"#));
    let composed = compose(&edition, &[base(), first]).unwrap();
    let id = PackageId::parse(D2).unwrap();
    assert_eq!(
        composed.changes,
        [
            Change {
                package: id.clone(),
                path: "segments.AA.elements.1.max".into(),
                kind: ChangeKind::Narrowed {
                    before: "2".into(),
                    after: "1".into(),
                },
            },
            Change {
                package: id,
                path: "segments.AA.elements.1.codes".into(),
                kind: ChangeKind::Widened {
                    before: r#"["X","Y"]"#.into(),
                    after: r#"["X","Z"]"#.into(),
                },
            },
        ]
    );
}

#[test]
fn a_patch_records_a_removal() {
    let patch = package(
        "test/patch/005010/835/X1",
        &["patch"],
        &[E],
        r#"{"segments":{"AA":{"elements":{"2":null}}}}"#,
    );
    let composed = compose(&patch, &[base()]).unwrap();
    assert_eq!(composed.changes.len(), 1);
    assert_eq!(composed.changes[0].kind, ChangeKind::Removed);
}

#[test]
fn widens_error_has_no_source() {
    let err = StackError::Widens {
        change: Box::new(Change {
            package: PackageId::parse(D).unwrap(),
            path: "x".into(),
            kind: ChangeKind::Removed,
        }),
    };
    assert!(std::error::Error::source(&err).is_none());
}

#[test]
fn every_builtin_composes_checked_to_the_model_of_the_registry() {
    let pool = PACKAGES.map(|text| Package::from_json(text).unwrap());
    for (builtin, top) in Builtin::all().iter().zip(&pool) {
        let checked = Stack::resolve(top, &pool)
            .unwrap()
            .compose_checked()
            .unwrap();
        assert_eq!(builtin.model().unwrap(), checked.model);
    }
}
