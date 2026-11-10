use serde_json::{from_str, from_value, json, to_string, to_value};

use crate::mask::Phi;
use crate::spec::raw::RawPick;
use crate::spec::vocabulary::{Vocabulary, words_text};
use crate::spec::*;
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

fn refused<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> String {
    from_value::<T>(value).err().unwrap().to_string()
}

fn refused_text<T: serde::de::DeserializeOwned>(json: &str) -> String {
    refused::<T>(from_str(json).unwrap())
}

fn bad_pick_text(found: &str) -> String {
    format!("\"pick\" must be \"first\", \"last\" or a 1-based position; found {found}")
}

#[test]
fn a_vocabulary_writes_and_reads_its_word() {
    assert_eq!(to_string(&Usage::Required).unwrap(), "\"required\"");
    assert_eq!(to_string(&Sign::Minus).unwrap(), "\"-\"");
    let count = from_str::<ControlCount>("\"children\"");
    assert_eq!(count.unwrap(), ControlCount::Children);
    assert_eq!(from_str::<Phi>("\"date\"").unwrap(), Phi::Date);
}

#[test]
fn an_unknown_word_is_refused_with_the_allowed_words() {
    let usage = refused::<Usage>(json!("often"));
    assert_eq!(
        usage,
        "\"usage\" must be \"required\" or \"situational\"; found \"often\""
    );
    assert!(from_value::<Usage>(json!("Required")).is_err());
    let sign = refused::<Sign>(json!("plus"));
    assert_eq!(sign, "\"sign\" must be \"+\" or \"-\"; found \"plus\"");
}

#[test]
fn a_pick_writes_and_reads_as_written() {
    let read = |value| from_value::<Pick>(value).unwrap();
    assert_eq!(to_value(Pick::First).unwrap(), json!("first"));
    assert_eq!(to_value(Pick::Nth(3)).unwrap(), json!(3));
    assert_eq!(read(json!("last")), Pick::Last);
    assert_eq!(read(json!(2)), Pick::Nth(2));
    assert_eq!(
        refused::<Pick>(json!("middle")),
        bad_pick_text("\"middle\"")
    );
    assert_eq!(refused::<Pick>(json!(0)), bad_pick_text("0"));
}

#[test]
fn a_pick_of_any_other_json_value_names_it() {
    let rows = [
        ("true", "true"),
        ("false", "false"),
        ("-1", "-1"),
        ("1.5", "1.5"),
        ("null", "null"),
        ("[1]", "an array"),
        ("{}", "an object"),
        ("18446744073709551616", "18446744073709552000"),
    ];
    for (json, found) in rows {
        assert_eq!(refused_text::<Pick>(json), bad_pick_text(found), "{json}");
        assert_eq!(
            refused_text::<RawPick>(json),
            bad_pick_text(found),
            "{json}"
        );
    }
}

#[test]
fn a_null_column_pick_reads_as_absent() {
    let column =
        from_value::<ColumnModel>(json!({"occurrence": "clp", "pick": null, "element": 4}));
    assert_eq!(column.unwrap().pick, None);
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
    type Codes = std::collections::BTreeMap<ElementPosition, Vec<String>>;
    let at = ElementPosition::parse("3-2").unwrap();
    let map = Codes::from([(at, vec!["A".to_string()])]);
    assert_eq!(to_string(&map).unwrap(), r#"{"3-2":["A"]}"#);
    assert_eq!(from_str::<Codes>(r#"{"3-2":["A"]}"#).unwrap(), map);
    for key in ["03", "3-0"] {
        let error = from_str::<Codes>(&format!(r#"{{"{key}":["A"]}}"#))
            .unwrap_err()
            .to_string();
        assert!(
            error.starts_with(&format!("codes key \"{key}\" must be \"<element>\" or \"<element>-<component>\", 1-based integers in canonical form")),
            "{error}"
        );
    }
}

#[test]
fn the_loader_texts_name_every_word() {
    let s = String::new;
    let has = |text: String, words: String| assert!(text.contains(&words), "{text}");
    let usage = words_text::<Usage>();
    has(
        OccurrenceError::UnknownUsage { found: s() }.to_string(),
        usage.clone(),
    );
    has(
        SpecError::UnknownLoopUsage {
            loop_name: s(),
            found: s(),
        }
        .to_string(),
        usage,
    );
    has(
        ControlError::UnknownCount { found: s() }.to_string(),
        words_text::<ControlCount>(),
    );
    has(
        BalanceError::UnknownSign {
            key: s(),
            found: s(),
        }
        .to_string(),
        words_text::<Sign>(),
    );
    has(
        TableDefError::UnknownPhi { given: s() }.to_string(),
        words_text::<Phi>(),
    );
    let bad_pick = TableDefError::BadPick { written: s() }.to_string();
    for (word, _) in Pick::NAMED {
        assert!(bad_pick.contains(&format!("\"{word}\"")), "{bad_pick}");
    }
}
