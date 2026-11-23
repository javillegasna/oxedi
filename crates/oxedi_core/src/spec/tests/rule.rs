//! The element rule at a place, with and without an occurrence's own list.

use crate::spec::{ElementRule, OccurrenceDef, Spec};

fn payer_identification(spec: &Spec) -> &OccurrenceDef {
    let id = spec.loop_id("1000A").unwrap();
    spec.get(id)
        .occurrences
        .iter()
        .find(|occurrence| occurrence.name == "payer_identification")
        .unwrap()
}

/// The rule at N1's first element, where `payer_identification` lists `PR`.
fn payer_rule(spec: &Spec) -> ElementRule<'_> {
    let first = spec.element_def(b"N1", 1, None).unwrap();
    ElementRule::at(first, Some(payer_identification(spec)), 1, None)
}

#[test]
fn an_element_alone_keeps_its_definition() {
    let spec = &Spec::builtin_835();
    let def = spec.element_def(b"NM1", 8, None).unwrap();
    let rule = ElementRule::of(def);
    assert_eq!(rule.required, def.required);
    assert_eq!(rule.length, def.length);
    assert_eq!(rule.codes, def.codes.as_slice());
    assert_eq!(rule.codes_of, None);
}

#[test]
fn an_occurrence_list_applies_at_its_place_only() {
    let spec = &Spec::builtin_835();
    let occurrence = payer_identification(spec);
    let rule = payer_rule(spec);
    assert_eq!(rule.codes, ["PR"]);
    assert_eq!(rule.codes_of, Some("payer_identification"));
    let second = spec.element_def(b"N1", 2, None).unwrap();
    let rule = ElementRule::at(second, Some(occurrence), 2, None);
    assert_eq!(rule.codes, second.codes.as_slice());
    assert_eq!(rule.codes_of, None);
}

#[test]
fn rejects_follows_the_list_that_applies() {
    let spec = &Spec::builtin_835();
    let rule = payer_rule(spec);
    assert!(rule.rejects(b"PE"));
    assert!(!rule.rejects(b"PR"));
    assert!(!ElementRule::of(rule.def).rejects(b"PE"));
}

#[test]
fn own_rejects_asks_the_occurrence_list_only() {
    let spec = &Spec::builtin_835();
    let rule = payer_rule(spec);
    assert!(rule.own_rejects(b"PE"));
    assert!(!rule.own_rejects(b"PR"));
    // Without an occurrence list the element's list is not asked.
    assert!(!ElementRule::of(rule.def).own_rejects(b"ZZ"));
    let second = spec.element_def(b"N1", 2, None).unwrap();
    let occurrence = payer_identification(spec);
    assert!(!ElementRule::at(second, Some(occurrence), 2, None).own_rejects(b"ZZ"));
}
