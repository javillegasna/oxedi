use std::collections::BTreeMap;

use serde_json::json;

use crate::mask::Phi;
use crate::spec::vocabulary::{Vocabulary, words_text};
use crate::spec::{
    BalanceError, ControlCount, ControlError, ElementPosition, OccurrenceError, Pick, Sign, Spec,
    SpecError, TableDefError, Usage,
};
use crate::value::ElementType;

fn words<V: Vocabulary>() -> Vec<&'static str> {
    V::ALL.iter().map(|value| value.word()).collect()
}

fn round_trips<V: Vocabulary + std::fmt::Debug>() {
    for &value in V::ALL {
        assert_eq!(V::from_word(value.word()), Some(value));
    }
}

#[test]
fn every_vocabulary_lists_its_words() {
    assert_eq!(words::<Usage>(), ["required", "situational"]);
    assert_eq!(words::<ControlCount>(), ["segments", "children"]);
    assert_eq!(words::<Sign>(), ["+", "-"]);
    assert_eq!(words::<Phi>(), ["name", "id", "date"]);
    round_trips::<Usage>();
    round_trips::<ControlCount>();
    round_trips::<Sign>();
    round_trips::<Phi>();
    assert_eq!(Usage::from_word("Required"), None);
    assert_eq!(Sign::from_word("plus"), None);
}

#[test]
fn a_codes_key_is_an_element_and_maybe_a_component() {
    let at = |element, component| Some(ElementPosition { element, component });
    assert_eq!(ElementPosition::parse("3"), at(3, None));
    assert_eq!(ElementPosition::parse("3-2"), at(3, Some(2)));
    for key in ["0", "03", "+3", "3-", "-2", "3-0", "3-02", "3-2-1", ""] {
        assert_eq!(ElementPosition::parse(key), None, "{key:?}");
    }
    assert_eq!(
        at(3, Some(2)).map(|p| p.to_string()).as_deref(),
        Some("3-2")
    );
    assert_eq!(at(3, None).map(|p| p.to_string()).as_deref(), Some("3"));
}

#[test]
fn a_pick_word_names_first_or_last() {
    assert_eq!(Pick::from_word("first"), Some(Pick::First));
    assert_eq!(Pick::from_word("last"), Some(Pick::Last));
    assert_eq!(Pick::from_word("middle"), None);
    assert_eq!(Pick::Last.to_string(), "\"last\"");
    assert_eq!(Pick::Nth(3).to_string(), "3");
}

#[test]
fn default_words() {
    assert_eq!(Usage::default(), Usage::Situational);
    assert_eq!(Sign::default(), Sign::Plus);
    assert_eq!(
        ElementType::parse("R", None),
        Ok(ElementType::R {
            scale: ElementType::DEFAULT_SCALE
        })
    );
}

#[test]
fn a_control_without_end_fails_before_its_count() {
    let json = r#"{"name":"t","loops":{"env":{"trigger":{"segment":"ST"},"control":{"opener_element":2,"closer_element":2,"count_element":1,"count":"rows"}}}}"#;
    assert!(matches!(
        Spec::from_json(json).unwrap_err(),
        SpecError::BadControl {
            reason: ControlError::NoEnd,
            ..
        }
    ));
}

#[test]
fn an_unnamed_occurrence_fails_before_its_usage() {
    let json = r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"},"occurrences":{"":{"segment":"AA","pos":0,"usage":"often"}}}}}"#;
    assert!(matches!(
        Spec::from_json(json).unwrap_err(),
        SpecError::BadOccurrence {
            reason,
            ..
        } if *reason == OccurrenceError::EmptyName
    ));
}

#[test]
fn a_null_usage_and_sign_read_as_the_default() {
    let json = r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"},"usage":null,
        "occurrences":{"aa":{"segment":"AA","pos":0,"usage":null},"a1":{"segment":"A1","pos":1}}}},
        "segments":{"A1":{"elements":{"1":{"name":"amount","type":"R"},"2":{"name":"other","type":"R"}}}},
        "balancing":{"b":{"per":"a",
            "target":[{"occurrence":"a1","elements":[1],"sign":null}],
            "sum":[{"occurrence":"a1","elements":[2],"sign":"-"}]}}}"#;
    let spec = Spec::from_json(json).unwrap();
    let def = spec.get(spec.loop_id("a").unwrap());
    assert_eq!(def.usage, Usage::Situational);
    assert_eq!(def.occurrences[0].usage, Usage::Situational);
    assert!(!spec.balancing()[0].target[0].negative);
}

#[test]
fn a_vocabulary_writes_and_reads_its_word() {
    assert_eq!(
        serde_json::to_string(&Usage::Required).unwrap(),
        "\"required\""
    );
    assert_eq!(serde_json::to_string(&Sign::Minus).unwrap(), "\"-\"");
    assert_eq!(
        serde_json::from_str::<ControlCount>("\"children\"").unwrap(),
        ControlCount::Children
    );
    assert_eq!(serde_json::from_str::<Phi>("\"date\"").unwrap(), Phi::Date);
}

#[test]
fn an_unknown_word_is_refused_with_the_allowed_words() {
    assert_eq!(
        serde_json::from_value::<Usage>(json!("often"))
            .unwrap_err()
            .to_string(),
        "\"usage\" must be \"required\" or \"situational\"; found \"often\""
    );
    assert!(serde_json::from_value::<Usage>(json!("Required")).is_err());
    assert_eq!(
        serde_json::from_value::<Sign>(json!("plus"))
            .unwrap_err()
            .to_string(),
        "\"sign\" must be \"+\" or \"-\"; found \"plus\""
    );
}

#[test]
fn a_pick_writes_and_reads_as_written() {
    assert_eq!(serde_json::to_value(Pick::First).unwrap(), json!("first"));
    assert_eq!(serde_json::to_value(Pick::Nth(3)).unwrap(), json!(3));
    assert_eq!(
        serde_json::from_value::<Pick>(json!("last")).unwrap(),
        Pick::Last
    );
    assert_eq!(
        serde_json::from_value::<Pick>(json!(2)).unwrap(),
        Pick::Nth(2)
    );
    assert_eq!(
        serde_json::from_value::<Pick>(json!("middle"))
            .unwrap_err()
            .to_string(),
        "\"pick\" must be \"first\", \"last\" or a 1-based position; found \"middle\""
    );
    assert_eq!(
        serde_json::from_value::<Pick>(json!(0))
            .unwrap_err()
            .to_string(),
        "\"pick\" must be \"first\", \"last\" or a 1-based position; found 0"
    );
}

#[test]
fn every_named_pick_prints_its_own_word() {
    for (word, pick) in Pick::NAMED {
        assert_eq!(Pick::from_word(word), Some(pick));
        assert_eq!(pick.to_string(), format!("\"{word}\""));
    }
}

#[test]
fn a_codes_key_is_a_map_key() {
    let map = BTreeMap::from([(
        ElementPosition {
            element: 3,
            component: Some(2),
        },
        vec!["A".to_string()],
    )]);
    let json = serde_json::to_string(&map).unwrap();
    assert_eq!(json, r#"{"3-2":["A"]}"#);
    assert_eq!(
        serde_json::from_str::<BTreeMap<ElementPosition, Vec<String>>>(&json).unwrap(),
        map
    );
    let error = serde_json::from_str::<BTreeMap<ElementPosition, Vec<String>>>(r#"{"03":["A"]}"#)
        .unwrap_err()
        .to_string();
    assert!(
        error.starts_with(
            "codes key \"03\" must be \"<element>\" or \"<element>-<component>\", 1-based integers in canonical form"
        ),
        "{error}"
    );
}

#[test]
fn the_loader_texts_name_every_word() {
    let own = String::new;
    assert!(
        OccurrenceError::UnknownUsage { found: own() }
            .to_string()
            .contains(&words_text::<Usage>())
    );
    assert!(
        SpecError::UnknownLoopUsage {
            loop_name: own(),
            found: own()
        }
        .to_string()
        .contains(&words_text::<Usage>())
    );
    assert!(
        ControlError::UnknownCount { found: own() }
            .to_string()
            .contains(&words_text::<ControlCount>())
    );
    assert!(
        BalanceError::UnknownSign {
            key: own(),
            found: own()
        }
        .to_string()
        .contains(&words_text::<Sign>())
    );
    assert!(
        TableDefError::UnknownPhi { given: own() }
            .to_string()
            .contains(&words_text::<Phi>())
    );
    let bad_pick = TableDefError::BadPick { written: own() }.to_string();
    for (word, _) in Pick::NAMED {
        assert!(bad_pick.contains(&format!("\"{word}\"")), "{bad_pick}");
    }
}
