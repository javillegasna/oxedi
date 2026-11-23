use super::super::*;
use super::{BASE, package};
use crate::spec_load::SpecError;
use crate::spec_load::tests::{package_of, shipped};

const E: &str = "test/envelope/005010";
const D: &str = "test/dictionary/005010";
const D2: &str = "test/dictionary/004010";

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
            element(r#"{"usage":"situational"}"#),
            r#"segments.AA.elements.1.usage: "required" becomes "situational""#,
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
            format!("package \"{D}\" does not only narrow what its bases allow: {change}{TAIL}"),
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
fn what_a_package_adds_is_recorded_only_for_an_edition() {
    let added = r#"{"segments":{"BB":{"elements":{"1":{"name":"n","type":"AN"}}}}}"#;
    let narrowing = dictionary(D, &[E], added);
    assert!(compose(&narrowing, &[base()]).unwrap().changes.is_empty());
    let first = dictionary(D, &[E], r#"{"segments":{}}"#);
    let edition = dictionary(D2, &[D], added);
    let changes = compose(&edition, &[base(), first]).unwrap().changes;
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].path, "segments.BB");
    assert_eq!(
        changes[0].to_string(),
        "segments.BB is added: {\"elements\":{\"1\":{\"name\":\"n\",\"type\":\"AN\"}}}"
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

const G: &str = "test/guide/005010/835/X1";

fn guide(spec: &str) -> Package {
    package(G, &["guide"], &[E], spec)
}

fn refusal(top: &Package) -> String {
    compose(top, &[base()]).unwrap_err().to_string()
}

#[test]
fn widening_a_loop_or_an_occurrence_is_refused() {
    let cases = [
        (
            r#"{"loops":{"a":{"usage":"situational"}}}"#,
            r#"loops.a.usage: "required" becomes "situational""#,
        ),
        (
            r#"{"loops":{"a":{"max":null}}}"#,
            "loops.a.max: 2 becomes no maximum",
        ),
        (
            r#"{"loops":{"a":{"occurrences":{"aa":{"elements":{"1":{"codes":["X","Y"]}}}}}}}"#,
            r#"loops.a.occurrences.aa.elements.1.codes: ["X"] becomes ["X","Y"]"#,
        ),
        (
            r#"{"loops":{"a":{"occurrences":{"ab":{"qualifier":{"codes":["X","Y"]}}}}}}"#,
            r#"loops.a.occurrences.ab.qualifier.codes: ["Y"] becomes ["X","Y"]"#,
        ),
    ];
    for (spec, change) in cases {
        assert_eq!(
            refusal(&guide(spec)),
            format!("package \"{G}\" does not only narrow what its bases allow: {change}{TAIL}"),
            "{spec}"
        );
    }
}

#[test]
fn widening_an_occurrence_entry_is_refused() {
    let entry = |change: &str| {
        format!(r#"{{"loops":{{"a":{{"occurrences":{{"aa":{{"elements":{{"1":{change}}}}}}}}}}}}}"#)
    };
    let at = "loops.a.occurrences.aa.elements.1";
    let cases = [
        (
            entry(r#"{"usage":"situational"}"#),
            format!(r#"{at}.usage: "required" becomes "situational""#),
        ),
        (entry(r#"{"max":2}"#), format!("{at}.max: 1 becomes 2")),
        (
            entry(r#"{"min":null}"#),
            format!("{at}.min: 1 becomes no minimum"),
        ),
        (
            entry(r#"{"codes":["X","Y"]}"#),
            format!(r#"{at}.codes: ["X"] becomes ["X","Y"]"#),
        ),
        (entry("null"), format!("{at} is removed")),
    ];
    for (spec, change) in cases {
        assert_eq!(
            refusal(&guide(&spec)),
            format!("package \"{G}\" does not only narrow what its bases allow: {change}{TAIL}"),
            "{spec}"
        );
    }
}

#[test]
fn narrowing_an_occurrence_entry_is_accepted() {
    let narrowed = r#"{"loops":{"a":{"occurrences":{"aa":{"elements":{"1":{"max":1}}}}}}}"#;
    let added = r#"{"loops":{"a":{"occurrences":{"aa":{"elements":{"2":{"max":3}}}}}}}"#;
    for spec in [narrowed, added] {
        let composed = compose(&guide(spec), &[base()]).unwrap();
        assert!(composed.changes.is_empty(), "{spec}");
    }
}

#[test]
fn a_package_that_changes_the_structure_is_refused() {
    let trigger = r#"{"loops":{"a":{"trigger":{"segment":"AA","where":{"1":"X"}}}}}"#;
    let top = package(
        "test/transaction-set/005010/835",
        &["transaction-set"],
        &[E],
        trigger,
    );
    assert_eq!(
        refusal(&top),
        format!(
            "package \"test/transaction-set/005010/835\" does not only narrow what its bases allow: \
             loops.a.trigger: {{\"segment\":\"AA\"}} becomes {{\"segment\":\"AA\",\"where\":{{\"1\":\"X\"}}}}{TAIL}"
        )
    );
}

#[test]
fn a_guide_changing_the_version_is_refused() {
    let top = guide(r#"{"version":{"values":["W"]}}"#);
    assert_eq!(
        refusal(&top),
        format!(
            "package \"{G}\" does not only narrow what its bases allow: version: \
             {{\"segment\":\"AA\",\"element\":1,\"values\":[\"V\"]}} becomes \
             {{\"segment\":\"AA\",\"element\":1,\"values\":[\"W\"]}}{TAIL}"
        )
    );
}

#[test]
fn a_views_edition_records_a_table_change() {
    let table = r#"{"tables":{"t":{"loops":["a"],"columns":{"c":{"segment":"AA","element":1}}}}}"#;
    let views = |id, bases: &[&str], spec| package(id, &["views"], bases, spec);
    let first = views("test/views/005010/835/X1", &[E], table);
    let renamed = r#"{"tables":{"t":{"columns":{"c":null,"d":{"segment":"AA","element":1}}}}}"#;
    let edition = views(
        "test/views/004010/835/X1",
        &["test/views/005010/835/X1"],
        renamed,
    );
    let first_composed = compose(&first, &[base()]).unwrap();
    assert!(first_composed.changes.is_empty());
    let composed = compose(&edition, &[base(), first]).unwrap();
    assert_eq!(composed.changes.len(), 1);
    assert_eq!(composed.changes[0].path, "tables.t");
    assert!(matches!(
        composed.changes[0].kind,
        ChangeKind::Changed { .. }
    ));
}

// Every difference of the 4010 edition from the 5010 package, in the order the
// comparison walks them: the path, and whether the edition narrowed, widened,
// changed, removed or added it.
const EDITION_CHANGES: &[(&str, &str)] = &[
    ("segments.AMT.elements.1.codes", "widened"),
    ("segments.CAS.elements.1.codes", "widened"),
    ("segments.CLP.elements.2.codes", "widened"),
    ("segments.CLP.elements.6.codes", "narrowed"),
    ("segments.CLP.elements.7.usage", "widened"),
    ("segments.CLP.elements.9.codes", "narrowed"),
    ("segments.CLP.elements.14", "removed"),
    ("segments.GS.elements.8.codes", "widened"),
    ("segments.ISA.elements.11.type", "changed"),
    ("segments.ISA.elements.11.codes", "narrowed"),
    ("segments.ISA.elements.12.codes", "widened"),
    ("segments.N1.elements.2.usage", "widened"),
    ("segments.N4.elements.2.usage", "narrowed"),
    ("segments.N4.elements.3.usage", "narrowed"),
    ("segments.N4.elements.7", "removed"),
    ("segments.NM1.elements.1.codes", "narrowed"),
    ("segments.NM1.elements.12", "removed"),
    ("segments.PER.elements.1.codes", "narrowed"),
    ("segments.PER.elements.3.codes", "narrowed"),
    ("segments.PER.elements.5.codes", "narrowed"),
    ("segments.PER.elements.7.codes", "narrowed"),
    ("segments.PLB.elements.3.composite.1.codes", "widened"),
    ("segments.PLB.elements.5.composite.1.codes", "widened"),
    ("segments.PLB.elements.7.composite.1.codes", "widened"),
    ("segments.PLB.elements.9.composite.1.codes", "widened"),
    ("segments.PLB.elements.11.composite.1.codes", "widened"),
    ("segments.PLB.elements.13.composite.1.codes", "widened"),
    ("segments.QTY.elements.1.codes", "widened"),
    ("segments.RDM", "removed"),
    ("segments.REF.elements.1.codes", "widened"),
    ("segments.ST.elements.3", "removed"),
    ("segments.SVC.elements.1.composite.1.codes", "widened"),
    ("segments.SVC.elements.1.composite.8", "removed"),
    ("segments.SVC.elements.6.composite.1.codes", "widened"),
    ("segments.SVC.elements.6.composite.8", "removed"),
    ("segments.TS3.elements.6", "added"),
    ("segments.TS3.elements.7", "added"),
    ("segments.TS3.elements.8", "added"),
    ("segments.TS3.elements.9", "added"),
    ("segments.TS3.elements.10", "added"),
    ("segments.TS3.elements.11", "added"),
    ("segments.TS3.elements.12", "added"),
    ("segments.TS3.elements.14", "added"),
    ("segments.TS3.elements.16", "added"),
    ("segments.TS3.elements.19", "added"),
    (
        "loops.1000A.occurrences.payer_business_contact_information.qualifier",
        "removed",
    ),
    (
        "loops.1000A.occurrences.payer_business_contact_information.elements.3",
        "removed",
    ),
    (
        "loops.1000A.occurrences.payer_business_contact_information.elements.5",
        "removed",
    ),
    (
        "loops.1000A.occurrences.payer_business_contact_information.elements.7",
        "removed",
    ),
    (
        "loops.1000A.occurrences.payer_technical_contact_information",
        "removed",
    ),
    ("loops.1000A.occurrences.payer_web_site", "removed"),
    (
        "loops.1000B.occurrences.payee_additional_identification.elements.1.codes",
        "widened",
    ),
    (
        "loops.1000B.occurrences.payee_identification.elements.3",
        "added",
    ),
    (
        "loops.1000B.occurrences.remittance_delivery_method",
        "removed",
    ),
    (
        "loops.2100.occurrences.claim_contact_information.max",
        "widened",
    ),
    (
        "loops.2100.occurrences.claim_contact_information.elements.1",
        "removed",
    ),
    (
        "loops.2100.occurrences.claim_contact_information.elements.3",
        "removed",
    ),
    (
        "loops.2100.occurrences.claim_contact_information.elements.5",
        "removed",
    ),
    (
        "loops.2100.occurrences.claim_contact_information.elements.7",
        "removed",
    ),
    ("loops.2100.occurrences.claim_received_date", "removed"),
    (
        "loops.2100.occurrences.claim_supplemental_information.max",
        "widened",
    ),
    (
        "loops.2100.occurrences.claim_supplemental_information.elements.1.codes",
        "widened",
    ),
    (
        "loops.2100.occurrences.claim_supplemental_information_quantity.max",
        "widened",
    ),
    (
        "loops.2100.occurrences.corrected_priority_payer_name.max",
        "widened",
    ),
    ("loops.2100.occurrences.coverage_expiration_date", "removed"),
    (
        "loops.2100.occurrences.insured_name.elements.8.codes",
        "widened",
    ),
    (
        "loops.2100.occurrences.other_claim_related_identification.qualifier.codes",
        "widened",
    ),
    ("loops.2100.occurrences.other_subscriber_name", "removed"),
    (
        "loops.2100.occurrences.rendering_provider_identification.qualifier.codes",
        "narrowed",
    ),
    (
        "loops.2100.occurrences.statement_from_or_to_date",
        "removed",
    ),
    ("loops.2100.occurrences.claim_date", "added"),
    (
        "loops.2110.occurrences.healthcare_policy_identification",
        "removed",
    ),
    ("loops.2110.occurrences.line_item_control_number", "removed"),
    (
        "loops.2110.occurrences.rendering_provider_information.qualifier.codes",
        "narrowed",
    ),
    ("loops.2110.occurrences.service_date.max", "widened"),
    (
        "loops.2110.occurrences.service_identification.max",
        "narrowed",
    ),
    (
        "loops.2110.occurrences.service_identification.qualifier.codes",
        "widened",
    ),
    (
        "loops.2110.occurrences.service_supplemental_amount.max",
        "widened",
    ),
    (
        "loops.2110.occurrences.service_supplemental_amount.elements.1.codes",
        "widened",
    ),
    (
        "loops.2110.occurrences.service_supplemental_quantity.elements.1.codes",
        "widened",
    ),
    ("version", "changed"),
];

fn word(kind: &ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Narrowed { .. } => "narrowed",
        ChangeKind::Widened { .. } => "widened",
        ChangeKind::Changed { .. } => "changed",
        ChangeKind::Removed => "removed",
        ChangeKind::Added { .. } => "added",
    }
}

#[test]
fn the_4010_edition_lists_its_changes() {
    let pool = shipped();
    let top = package_of(&pool, "oxedi/guide/004010/835/X091A1");
    let composed = Stack::resolve(top, &pool)
        .unwrap()
        .compose_checked()
        .unwrap();
    let id = "oxedi/guide/004010/835/X091A1";
    assert!(composed.changes.iter().all(|c| c.package.as_str() == id));
    let listed: Vec<(&str, &str)> = composed
        .changes
        .iter()
        .map(|change| (change.path.as_str(), word(&change.kind)))
        .collect();
    assert_eq!(listed, EDITION_CHANGES);
    let isa = composed
        .changes
        .iter()
        .find(|c| c.path == "segments.ISA.elements.11.type");
    let changed = ChangeKind::Changed {
        before: r#""AN""#.into(),
        after: r#""ID""#.into(),
    };
    assert_eq!(isa.map(|c| &c.kind), Some(&changed));
}
