//! Tests of `validate`: the answer for the shipped stacks and for each way a
//! stack fails to load.

use super::{
    CannotWrite, Capability, Invalid, NoPatientData, Reasons, Text, Valid, Validation, validate,
};
use crate::spec_load::{Builtin, Role};
use crate::write::{PlanError, Refusal};

fn shipped() -> &'static [&'static str] {
    Builtin::packages()
}

fn yes(validation: Validation) -> Valid {
    match validation {
        Validation::Yes(valid) => *valid,
        Validation::No(invalid) => panic!("expected yes, found: {invalid}"),
    }
}

fn no(validation: Validation) -> Invalid {
    match validation {
        Validation::Yes(valid) => panic!("expected no, found: {valid}"),
        Validation::No(invalid) => *invalid,
    }
}

#[test]
fn the_5010_stack_says_yes() {
    let valid = yes(validate(shipped()[4], shipped()));
    assert_eq!(valid.id().as_str(), "oxedi/views/005010/835/X221A1");
    assert_eq!(valid.name(), "835");
    let ids: Vec<&str> = valid.members().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "oxedi/envelope/005010",
            "oxedi/dictionary/005010",
            "oxedi/transaction-set/005010/835",
            "oxedi/guide/005010/835/X221A1",
            "oxedi/views/005010/835/X221A1",
        ]
    );
    assert!(valid.members().all(|m| m.role == Role::Narrows));
    assert!(valid.changes.is_empty());
    assert!(valid.write.can() && valid.mask.can());
    assert_eq!(
        valid.to_string(),
        "yes: stack \"oxedi/views/005010/835/X221A1\" loads as spec \"835\"\npackages, bases \
         first: \"oxedi/envelope/005010\" (narrows), \"oxedi/dictionary/005010\" (narrows), \
         \"oxedi/transaction-set/005010/835\" (narrows), \"oxedi/guide/005010/835/X221A1\" \
         (narrows), \"oxedi/views/005010/835/X221A1\" (narrows)\ncan read\ncan write\ncan mask\nchanges: none"
    );
}

#[test]
fn the_4010_stack_says_yes_and_lists_its_changes() {
    let valid = yes(validate(shipped()[7], shipped()));
    assert_eq!(valid.members().count(), 8);
    let last = Some(valid.top.id.as_str());
    assert_eq!(last, Some("oxedi/guide/004010/835/X091A1"));
    assert_eq!(valid.changes.len(), 80);
    assert!(
        valid.change_lines()[0].ends_with("(package \"oxedi/envelope/004010\")"),
        "{}",
        valid.change_lines()[0]
    );
    let text = valid.to_string();
    assert!(
        text.starts_with("yes: stack \"oxedi/guide/004010/835/X091A1\" loads as spec \"835\"\n")
    );
    assert!(text.contains("\nchanges (80):\n1. "), "{text}");
}

#[test]
fn every_builtin_says_yes() {
    for builtin in Builtin::all() {
        let top = shipped()
            .iter()
            .find(|text| text.contains(&format!("\"id\": \"{}\"", builtin.id)))
            .copied()
            .unwrap_or_else(|| panic!("no shipped text for {}", builtin.id));
        let valid = yes(validate(top, shipped()));
        assert_eq!(*valid.id(), builtin.id);
        assert_eq!(valid.loaded.spec.loops(), builtin.spec.loops());
        assert_eq!(valid.loaded.spec.tables(), builtin.spec.tables());
    }
}

#[test]
fn a_top_that_is_not_json_says_no() {
    let invalid = no(validate("{", &[]));
    assert!(matches!(
        invalid,
        Invalid::Unreadable {
            text: Text::Top,
            ..
        }
    ));
    assert_eq!(
        invalid.to_string(),
        "no: the top package does not read: invalid spec JSON: EOF while parsing an object at \
         line 1 column 1"
    );
}

#[test]
fn a_pool_text_with_a_newer_format_says_no_naming_it() {
    let newer = shipped()[1].replacen("\"format\": 1", "\"format\": 3", 1);
    let invalid = no(validate(shipped()[4], &[shipped()[0], &newer]));
    assert_eq!(
        invalid.to_string(),
        "no: package 2 of the pool does not read: the package header is invalid: package \
         format 3 is newer than 2, the newest format this reader takes; read the package with \
         a newer oxedi, or write it in format 2"
    );
}

#[test]
fn an_unknown_base_says_no() {
    let top = r#"{"package":{"format":1,"id":"t/views/005010/835/X1","layers":["views"],"bases":["t/guide/005010/835/X1"]},"spec":{}}"#;
    let invalid = no(validate(top, &[]));
    assert_eq!(
        invalid.to_string(),
        "no: stack \"t/views/005010/835/X1\" does not load: package \"t/views/005010/835/X1\" \
         builds on \"t/guide/005010/835/X1\", which is not among the packages given; give \
         that package too, or remove it from \"bases\""
    );
}

#[test]
fn a_package_that_widens_its_base_says_no() {
    let top = r#"{"package":{"format":1,"id":"t/dictionary/005010","layers":["dictionary"],"bases":["oxedi/envelope/005010"]},"spec":{"segments":{"ISA":{"elements":{"1":{"max":3}}}}}}"#;
    let invalid = no(validate(top, &shipped()[..1]));
    assert!(
        invalid.to_string().starts_with(
            "no: stack \"t/dictionary/005010\" does not load: package \"t/dictionary/005010\" \
             does not only narrow what its bases allow: "
        ),
        "{invalid}"
    );
}

#[test]
fn a_stack_that_does_not_compile_says_no() {
    let top = r#"{"package":{"format":1,"id":"t/views/005010/835/X1","layers":["views"],"bases":["oxedi/guide/005010/835/X221A1"]},"spec":{"tables":{"t":{"loops":["nope"],"columns":{"c":{"segment":"BPR","element":1}}}}}}"#;
    let invalid = no(validate(top, &shipped()[..4]));
    assert!(matches!(invalid, Invalid::Unloadable { .. }));
    let text = invalid.to_string();
    assert!(
        text.starts_with("no: stack \"t/views/005010/835/X1\" does not load: "),
        "{text}"
    );
    assert!(text.contains("\"nope\""), "{text}");
}

#[test]
fn a_pool_that_holds_two_packages_with_one_id_says_no() {
    let invalid = no(validate(shipped()[4], &[shipped()[0], shipped()[0]]));
    assert_eq!(
        invalid.to_string(),
        "no: stack \"oxedi/views/005010/835/X221A1\" does not load: two of the packages given \
         are named \"oxedi/envelope/005010\"; give each package once"
    );
}

#[test]
fn a_pool_package_the_stack_does_not_use_is_ignored() {
    let pool = [
        shipped()[0],
        shipped()[1],
        shipped()[2],
        shipped()[3],
        shipped()[5],
    ];
    let valid = yes(validate(shipped()[4], &pool));
    let ids: Vec<&str> = valid.members().map(|m| m.id.as_str()).collect();
    assert_eq!(ids.len(), 5);
    assert!(!ids.contains(&"oxedi/envelope/004010"), "{ids:?}");
    assert_eq!(valid.id().as_str(), "oxedi/views/005010/835/X221A1");
}

#[test]
fn the_4010_stack_can_write_and_mask() {
    let valid = yes(validate(shipped()[7], shipped()));
    assert!(valid.write.can());
    assert!(valid.mask.can());
}

#[test]
fn a_guide_top_cannot_write_or_mask() {
    let valid = yes(validate(shipped()[3], shipped()));
    assert_eq!(valid.write, Capability::Cannot(CannotWrite::NoTables));
    assert_eq!(
        valid.write.reasons(),
        vec!["the stack has no tables; build a views package on it"]
    );
    assert_eq!(
        valid.mask.reasons(),
        vec![
            "no column of any table is flagged \"phi\"; flag the columns that hold names, ids \
             or dates with \"phi\": \"name\", \"id\" or \"date\""
        ]
    );
    let text = valid.to_string();
    assert!(
        text.contains("\ncannot mask (1 reason):\n1. no column of any table is flagged"),
        "{text}"
    );
    assert!(
        text.contains(
            "\ncannot write (1 reason):\n1. the stack has no tables; build a views package on it\n"
        ),
        "{text}"
    );
}

#[test]
fn write_reasons_say_what_to_do() {
    let plan = PlanError {
        spec: "s".into(),
        refusals: vec![Refusal::SeveralAnchorLoops {
            table: "names".into(),
            loops: vec!["1000A".into(), "1000B".into()],
        }],
    };
    assert_eq!(
        CannotWrite::Refused(plan).lines(),
        vec![
            "table \"names\" is anchored on 2 loops (\"1000A\", \"1000B\"); a written row opens one \
             loop instance and nothing in the row says which; anchor the table on one loop"
        ]
    );
}

#[test]
fn a_capability_that_can_has_no_reasons() {
    assert!(Capability::<NoPatientData>::Can.reasons().is_empty());
}

#[test]
fn every_builtin_says_yes_with_its_capabilities() {
    let compat = include_str!("../../specs/edi_835_parser.json");
    for top in [shipped()[4], shipped()[7]] {
        let valid = yes(validate(top, shipped()));
        assert!(valid.write.can() && valid.mask.can(), "{}", valid.id());
    }
    let valid = yes(validate(compat, shipped()));
    assert_eq!(
        valid.id().as_str(),
        "edi-835-parser/patch/005010/835/X221A1"
    );
    assert!(!valid.write.can());
    let write = valid.write.reasons();
    assert_eq!(write.len(), 17);
    assert_eq!(
        write[0],
        "table \"rows_organizations\" is anchored on 2 loops (\"1000A\", \"1000B\"); a written \
         row opens one loop instance and nothing in the row says which; anchor the table on one loop"
    );
    assert!(!valid.mask.can());
    assert_eq!(
        valid.mask.reasons(),
        [
            "no column of any table is flagged \"phi\"; flag the columns that hold names, ids or \
             dates with \"phi\": \"name\", \"id\" or \"date\""
        ]
    );
}
