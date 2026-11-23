use serde_json::json;

use super::super::*;

fn parse_err(text: &str) -> IdError {
    PackageId::parse(text).unwrap_err()
}

#[test]
fn each_layer_reads_its_own_form() {
    let cases = [
        ("oxedi/envelope/005010", Layer::Envelope, None, None),
        ("oxedi/dictionary/004010", Layer::Dictionary, None, None),
        (
            "oxedi/transaction-set/005010/835",
            Layer::TransactionSet,
            Some("835"),
            None,
        ),
        (
            "oxedi/guide/005010/835/X221A1",
            Layer::Guide,
            Some("835"),
            Some("X221A1"),
        ),
        (
            "oxedi/views/005010/835/X221A1",
            Layer::Views,
            Some("835"),
            Some("X221A1"),
        ),
        (
            "acme-1/patch/005010/835/X221A1",
            Layer::Patch,
            Some("835"),
            Some("X221A1"),
        ),
    ];
    for (text, layer, set, guide) in cases {
        let id = PackageId::parse(text).unwrap();
        assert_eq!(id.layer(), layer, "{text}");
        assert_eq!(id.release().as_str(), text.split('/').nth(2).unwrap());
        assert_eq!(id.transaction_set().map(TransactionSet::as_str), set);
        assert_eq!(id.guide().map(Guide::as_str), guide);
        assert_eq!(id.author(), text.split('/').next().unwrap());
        assert_eq!(id.to_string(), text);
    }
}

#[test]
fn spec_name_is_the_set_else_the_release() {
    let guide = PackageId::parse("oxedi/guide/005010/835/X221A1").unwrap();
    assert_eq!(guide.spec_name(), "835");
    let envelope = PackageId::parse("oxedi/envelope/005010").unwrap();
    assert_eq!(envelope.spec_name(), "005010");
}

#[test]
fn parts_display() {
    assert_eq!(
        parse_err("oxedi/guide/005010/835").to_string(),
        "package id \"oxedi/guide/005010/835\" is not written <author>/guide/<release>/<transaction set>/<guide>"
    );
    assert_eq!(
        parse_err("oxedi").to_string(),
        "package id \"oxedi\" is not written <author>/<layer>/<release>[/<transaction set>[/<guide>]]"
    );
}

#[test]
fn layer_display() {
    assert_eq!(
        parse_err("oxedi/glossary/005010").to_string(),
        "package id \"oxedi/glossary/005010\": layer \"glossary\" must be \"envelope\", \"dictionary\", \"transaction-set\", \"guide\", \"views\" or \"patch\""
    );
}

#[test]
fn part_display() {
    assert_eq!(
        parse_err("Oxedi/envelope/005010").to_string(),
        "package id \"Oxedi/envelope/005010\": author \"Oxedi\" must be lowercase ASCII letters, digits and \"-\", starting with a letter, at most 32, e.g. \"oxedi\""
    );
}

#[test]
fn each_part_rule_refuses_its_breaks() {
    let long_author = format!("{}/envelope/005010", "a".repeat(33));
    let cases = [
        ("oxedi/envelope/5010", IdPart::Release, "5010"),
        (
            "oxedi/transaction-set/005010/83A",
            IdPart::TransactionSet,
            "83A",
        ),
        ("oxedi/guide/005010/835/x221a1", IdPart::Guide, "x221a1"),
        ("oxedi/guide/005010/835/1X", IdPart::Guide, "1X"),
        ("-x/envelope/005010", IdPart::Author, "-x"),
        (long_author.as_str(), IdPart::Author, &long_author[..33]),
    ];
    for (id, part, text) in cases {
        assert_eq!(
            parse_err(id),
            IdError::Part {
                id: id.to_string(),
                part,
                text: text.to_string()
            },
            "{id}"
        );
    }
}

#[test]
fn an_id_with_a_part_missing_or_misplaced_is_refused_in_full() {
    let cases = [
        (
            "/envelope/005010",
            "package id \"/envelope/005010\": author \"\" must be lowercase ASCII letters, digits and \"-\", starting with a letter, at most 32, e.g. \"oxedi\"",
        ),
        (
            "oxedi//005010",
            "package id \"oxedi//005010\": layer \"\" must be \"envelope\", \"dictionary\", \"transaction-set\", \"guide\", \"views\" or \"patch\"",
        ),
        (
            "oxedi/envelope/005010/",
            "package id \"oxedi/envelope/005010/\" is not written <author>/envelope/<release>",
        ),
        (
            "oxedi/envelope/005010/835",
            "package id \"oxedi/envelope/005010/835\" is not written <author>/envelope/<release>",
        ),
        (
            "ox\u{e9}di/envelope/005010",
            "package id \"ox\u{e9}di/envelope/005010\": author \"ox\u{e9}di\" must be lowercase ASCII letters, digits and \"-\", starting with a letter, at most 32, e.g. \"oxedi\"",
        ),
    ];
    for (id, text) in cases {
        assert_eq!(parse_err(id).to_string(), text, "{id}");
    }
}

#[test]
fn an_id_reads_through_serde_by_its_rule() {
    let err = serde_json::from_value::<PackageId>(json!("oxedi/x/005010")).unwrap_err();
    assert_eq!(err.to_string(), parse_err("oxedi/x/005010").to_string());
}

#[test]
fn layer_words_round_trip() {
    for layer in Layer::ALL {
        assert_eq!(Layer::parse(layer.word()), Some(layer));
    }
    assert!(Layer::ALL.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn id_error_has_no_source() {
    assert!(std::error::Error::source(&parse_err("oxedi")).is_none());
}
