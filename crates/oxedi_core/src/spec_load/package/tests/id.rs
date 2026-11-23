use super::super::*;

#[test]
fn layer_display() {
    let err = IdError::Layer {
        id: "oxedi/glossary/005010".into(),
        layer: "glossary".into(),
    };
    assert_eq!(
        err.to_string(),
        "package id \"oxedi/glossary/005010\": layer \"glossary\" must be \"envelope\", \"dictionary\", \"transaction-set\", \"guide\", \"views\" or \"patch\""
    );
}

#[test]
fn parts_display() {
    let err = IdError::Parts {
        id: "oxedi".into(),
        form: "<author>/<layer>/<release>[/<transaction set>[/<guide>]]",
    };
    assert_eq!(
        err.to_string(),
        "package id \"oxedi\" is not written <author>/<layer>/<release>[/<transaction set>[/<guide>]]"
    );
}

#[test]
fn part_display() {
    let err = IdError::Part {
        id: "Oxedi/envelope/005010".into(),
        part: IdPart::Author,
        text: "Oxedi".into(),
    };
    assert_eq!(
        err.to_string(),
        "package id \"Oxedi/envelope/005010\": author \"Oxedi\" must be lowercase ASCII letters, digits and \"-\", starting with a letter, at most 32, e.g. \"oxedi\""
    );
}

#[test]
fn each_part_has_a_name_and_a_rule() {
    let names = [
        (IdPart::Author, "author"),
        (IdPart::Release, "release"),
        (IdPart::TransactionSet, "transaction set"),
        (IdPart::Guide, "guide"),
    ];
    for (part, name) in names {
        assert_eq!(part.name(), name);
        assert!(part.rule().contains("e.g."));
    }
}

#[test]
fn layer_words_round_trip() {
    for layer in Layer::ALL {
        assert_eq!(Layer::parse(layer.word()), Some(layer));
        assert_eq!(layer.to_string(), layer.word());
    }
    assert_eq!(Layer::parse("glossary"), None);
    assert!(Layer::ALL.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn id_error_has_no_source() {
    let err = IdError::Parts {
        id: "oxedi".into(),
        form: "x",
    };
    assert!(std::error::Error::source(&err).is_none());
}
