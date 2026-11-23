use serde_json::json;

use super::super::stack::{Role, Stack};
use super::super::*;
use super::{envelope, package};
use crate::spec_load::SpecError;

const E: &str = "test/envelope/005010";
const D: &str = "test/dictionary/005010";
const G: &str = "test/guide/005010/835/X1";

fn guide(spec: &str) -> Package {
    package(G, &["guide"], &[E], spec)
}

fn ids(stack: &Stack<'_>) -> Vec<String> {
    stack
        .steps()
        .map(|step| step.package.id().to_string())
        .collect()
}

#[test]
fn a_stack_lists_bases_first_each_once() {
    let transaction_set = package(
        "test/transaction-set/005010/835",
        &["transaction-set"],
        &[D, E],
        "{}",
    );
    let pool = [
        envelope(),
        package(D, &["dictionary"], &[E], "{}"),
        transaction_set.clone(),
    ];
    let stack = Stack::resolve(&transaction_set, &pool).unwrap();
    assert_eq!(ids(&stack), [E, D, "test/transaction-set/005010/835"]);
}

#[test]
fn an_unknown_base_is_refused() {
    let dictionary = package(D, &["dictionary"], &[E], "{}");
    let err = Stack::resolve(&dictionary, std::slice::from_ref(&dictionary)).unwrap_err();
    assert!(matches!(
        &err,
        SpecError::Stack { reason } if matches!(**reason, StackError::UnknownBase { .. })
    ));
    assert_eq!(
        err.to_string(),
        "package \"test/dictionary/005010\" builds on \"test/envelope/005010\", which is not among the packages given; give that package too, or remove it from \"bases\""
    );
}

#[test]
fn a_base_cycle_is_refused() {
    let first = package(E, &["envelope"], &[D], "{}");
    let second = package(D, &["dictionary"], &[E], "{}");
    let err = Stack::resolve(&first, &[first.clone(), second]).unwrap_err();
    assert_eq!(
        err.to_string(),
        "packages form a base cycle: test/envelope/005010 -> test/dictionary/005010 -> test/envelope/005010; a package cannot build on itself"
    );
}

#[test]
fn a_package_that_builds_on_itself_is_a_cycle() {
    let own = package(E, &["envelope"], &[E], "{}");
    let err = Stack::resolve(&own, std::slice::from_ref(&own)).unwrap_err();
    assert_eq!(
        err.to_string(),
        "packages form a base cycle: test/envelope/005010 -> test/envelope/005010; a package cannot build on itself"
    );
}

#[test]
fn two_packages_with_one_id_are_refused() {
    let pool = [envelope(), envelope()];
    let err = Stack::resolve(&pool[0], &pool).unwrap_err();
    assert!(matches!(
        &err,
        SpecError::Stack { reason } if matches!(**reason, StackError::DuplicatePackage { .. })
    ));
    assert_eq!(
        err.to_string(),
        "two of the packages given are named \"test/envelope/005010\"; give each package once"
    );
}

#[test]
fn a_fault_in_a_base_names_the_base() {
    let dictionary = package(
        D,
        &["dictionary"],
        &[E],
        r#"{"loops":{"a":{"occurrences":{"aa":{"usage":"sometimes"}}}}}"#,
    );
    let top = package(G, &["guide"], &[D], "{}");
    let pool = [envelope(), dictionary, top.clone()];
    let err = Stack::resolve(&top, &pool).unwrap().compose().unwrap_err();
    assert_eq!(
        err.to_string(),
        "package \"test/dictionary/005010\": spec: at loops.a.occurrences.aa.usage: \"usage\" must be \"required\" or \"situational\"; found \"sometimes\""
    );
}

#[test]
fn the_name_comes_from_the_top_id() {
    let envelope = envelope();
    let transaction_set = package(
        "test/transaction-set/005010/835",
        &["transaction-set"],
        &[E],
        "{}",
    );
    let pool = [envelope.clone(), transaction_set.clone()];
    let stack = Stack::resolve(&transaction_set, &pool).unwrap();
    assert_eq!(stack.compose().unwrap().model.name, "835");
    let alone = Stack::resolve(&envelope, &pool).unwrap();
    assert_eq!(alone.compose().unwrap().model.name, "005010");
}

#[test]
fn packages_merge_by_value_in_order() {
    let guided = guide(
        r#"{"loops":{"a":{"usage":"required","occurrences":{"aa":{"usage":"required"},"ab":{"segment":"AB","pos":1}}}},"segments":{"AA":{"elements":{"1":{"codes":["X"]}}}}}"#,
    );
    let pool = [envelope(), guided.clone()];
    let model = Stack::resolve(&guided, &pool)
        .unwrap()
        .compose()
        .unwrap()
        .model;
    let value = serde_json::to_value(&model).unwrap();
    let occurrences = &value["loops"]["a"]["occurrences"];
    assert_eq!(occurrences["aa"]["segment"], "AA");
    assert_eq!(occurrences["aa"]["usage"], "required");
    assert_eq!(occurrences["ab"]["segment"], "AB");
    assert_eq!(
        value["segments"]["AA"]["elements"]["1"]["codes"],
        json!(["X"])
    );

    let patch = package(
        "test/patch/005010/835/X1",
        &["patch"],
        &[G],
        r#"{"loops":{"a":{"occurrences":{"ab":null}}}}"#,
    );
    let pool = [envelope(), guided, patch.clone()];
    let value = serde_json::to_value(
        Stack::resolve(&patch, &pool)
            .unwrap()
            .compose()
            .unwrap()
            .model,
    )
    .unwrap();
    assert!(value["loops"]["a"]["occurrences"].get("ab").is_none());
}

#[test]
fn a_refusal_after_a_package_names_it_and_the_path() {
    let guided = guide(r#"{"loops":{"a":{"occurrences":{"aa":{"usage":"sometimes"}}}}}"#);
    let pool = [envelope(), guided.clone()];
    let err = Stack::resolve(&guided, &pool)
        .unwrap()
        .compose()
        .unwrap_err();
    assert!(matches!(
        &err,
        SpecError::Package { id, source }
            if id.as_str() == G && matches!(**source, SpecError::Format { .. })
    ));
    assert_eq!(
        err.to_string(),
        "package \"test/guide/005010/835/X1\": spec: at loops.a.occurrences.aa.usage: \"usage\" must be \"required\" or \"situational\"; found \"sometimes\""
    );
    assert!(std::error::Error::source(&err).is_none());
}

#[test]
fn the_first_package_must_read_as_a_whole_model() {
    let dictionary = package(D, &["dictionary"], &[], r#"{"segments":{}}"#);
    let err = Stack::resolve(&dictionary, std::slice::from_ref(&dictionary))
        .unwrap()
        .compose()
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "package \"test/dictionary/005010\": spec: at .: missing field `loops`"
    );
}

#[test]
fn stack_error_has_no_source() {
    let err = StackError::BaseCycle {
        members: vec![PackageId::parse(E).unwrap()],
    };
    assert!(std::error::Error::source(&err).is_none());
    assert!(
        std::error::Error::source(&SpecError::Stack {
            reason: Box::new(err)
        })
        .is_none()
    );
}

fn roles(top: &Package, pool: &[Package]) -> Vec<Role> {
    Stack::resolve(top, pool)
        .unwrap()
        .steps()
        .map(|step| step.role)
        .collect()
}

#[test]
fn a_package_narrows_shares_a_layer_or_patches() {
    let dictionary = package(D, &["dictionary"], &[E], "{}");
    let edition = package("acme/dictionary/005010", &["dictionary"], &[D], "{}");
    let patch = package("acme/patch/005010/835/X1", &["patch"], &[E], "{}");
    let pool = [envelope(), dictionary, edition.clone(), patch.clone()];
    assert_eq!(
        roles(&edition, &pool),
        [Role::Narrows, Role::Narrows, Role::Edition]
    );
    assert_eq!(roles(&patch, &pool), [Role::Narrows, Role::Patch]);
}

#[test]
fn a_package_must_come_after_the_layers_it_builds_on() {
    let transaction_set = package(
        "test/transaction-set/005010/835",
        &["transaction-set"],
        &[G],
        "{}",
    );
    let guide = package(G, &["guide"], &[E], "{}");
    let pool = [envelope(), guide, transaction_set.clone()];
    let err = Stack::resolve(&transaction_set, &pool).unwrap_err();
    assert_eq!(
        err.to_string(),
        "package \"test/transaction-set/005010/835\" holds layer \"transaction-set\", which does not come after layer \"guide\" of its base \"test/guide/005010/835/X1\"; a package builds only on lower layers, unless it is an edition (it holds a layer its base holds) or a patch"
    );
}
