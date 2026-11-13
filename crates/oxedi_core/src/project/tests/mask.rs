use crate::project::mask::{date_datum, masked_date};
use crate::project::quote::unquoted;
use crate::project::*;
use crate::{DateMode, Document, Mask, Phi, Rule, SnipLevel};

const SAMPLE: &[u8] = include_bytes!("../../../tests/fixtures/balanced_5010_sample.txt");
const KEY: &[u8] = b"oxedi-test-key-0";

fn mask() -> Mask {
    Mask::new(KEY, DateMode::Year).unwrap()
}

/// The index of the first segment `id` of `SAMPLE` whose first element is
/// `first`, and that element.
fn find(id: &[u8], first: Option<&[u8]>) -> (usize, Vec<u8>) {
    let document = Document::parse(SAMPLE).unwrap();
    document
        .segments()
        .find_map(|segment| {
            let value = segment.element(1)?.simple()?.to_vec();
            (segment.id == id && first.is_none_or(|first| first == value.as_slice()))
                .then_some((segment.index, value))
        })
        .unwrap()
}

/// The first `CLP` of `SAMPLE`: its index, its first element and that
/// element as text.
fn first_claim() -> (usize, Vec<u8>, String) {
    let (index, id) = find(b"CLP", None);
    let text = String::from_utf8(id.clone()).unwrap();
    (index, id, text)
}

fn external(message: String, segment: Option<usize>, element: usize, datum: &[u8]) -> Diagnostic {
    Diagnostic::new(
        Rule::External {
            origin: "pyx12".into(),
            code: Some("8".into()),
            message,
            level: SnipLevel::L2,
        },
        segment,
        Some(element),
        None,
        vec![],
        datum.to_vec(),
    )
}

/// A finding on a whole segment: no element position.
fn segment_level(message: String, segment: usize, datum: &[u8]) -> Diagnostic {
    Diagnostic::new(
        Rule::External {
            origin: "pyx12".into(),
            code: Some("1".into()),
            message,
            level: SnipLevel::L2,
        },
        Some(segment),
        None,
        None,
        vec![],
        datum.to_vec(),
    )
}

fn message(diagnostic: &Diagnostic) -> &str {
    match &diagnostic.rule {
        Rule::External { message, .. } => message,
        _ => panic!("not an external finding"),
    }
}

/// The findings `Projector::mask_found` leaves for `SAMPLE`, with the
/// builtin 835 spec and `mask` when there is one.
fn mask_found(mut found: Vec<Diagnostic>, mask: Option<&Mask>) -> Vec<Diagnostic> {
    let spec = Spec::builtin_835();
    let document = Document::parse(SAMPLE).unwrap();
    let projector = Projector::new(&spec, document.delimiters());
    let projector = match mask {
        Some(mask) => projector.with_mask(mask),
        None => projector,
    };
    projector.mask_found(&mut found, &document);
    found
}

/// The masked text of `value` as the mask writes it for `phi`.
fn token(phi: Phi, value: &[u8]) -> String {
    String::from_utf8_lossy(&mask().text(phi, value)).into_owned()
}

/// The findings come out of masking exactly as they went in.
fn assert_unchanged(found: Vec<Diagnostic>, mask: Option<&Mask>) {
    assert_eq!(mask_found(found.clone(), mask), found);
}

#[test]
fn mask_found_masks_the_datum_and_message_of_a_finding_on_a_flagged_place() {
    let (index, id, text) = first_claim();
    let found = vec![external(
        format!("Invalid value '{text}' in CLP01"),
        Some(index),
        1,
        &id,
    )];

    let found = mask_found(found, Some(&mask()));

    let masked = token(Phi::Id, &id);
    assert_eq!(found[0].datum, masked.as_bytes());
    assert!(!message(&found[0]).contains(&text));
    assert_eq!(
        message(&found[0]),
        format!("Invalid value '{masked}' in CLP01")
    );
}

#[test]
fn mask_found_leaves_a_finding_on_an_unflagged_place_as_written() {
    let (index, _) = find(b"N1", Some(b"PR"));
    let original = external(
        "Invalid value 'ACME HEALTH PLAN' in N102".into(),
        Some(index),
        2,
        b"ACME HEALTH PLAN",
    );

    assert_unchanged(vec![original], Some(&mask()));
}

#[test]
fn mask_found_leaves_a_finding_without_a_segment_or_past_the_end_as_written() {
    let (_, id, text) = first_claim();
    let originals = vec![
        external(format!("Invalid value '{text}'"), None, 1, &id),
        external(format!("Invalid value '{text}'"), Some(usize::MAX), 1, &id),
    ];

    assert_unchanged(originals, Some(&mask()));
}

#[test]
fn mask_found_does_nothing_without_a_mask() {
    let (index, id, text) = first_claim();
    let original = external(format!("Invalid value '{text}'"), Some(index), 1, &id);

    assert_unchanged(vec![original], None);
}

#[test]
fn mask_found_replaces_every_occurrence_of_a_one_character_datum() {
    let (index, _) = find(b"CLP", None);
    let found = vec![external("1 of 1".into(), Some(index), 1, b"1")];

    let found = mask_found(found, Some(&mask()));

    let masked = token(Phi::Id, b"1");
    assert_eq!(found[0].datum, masked.as_bytes());
    assert_eq!(message(&found[0]), format!("{masked} of {masked}"));
}

#[test]
fn mask_found_replaces_a_datum_quoted_as_latin_1_text() {
    let (index, _) = find(b"NM1", Some(b"QC"));
    let datum = b"D\xe9OE";
    let found = vec![external(
        "contains an invalid character(D\u{e9}OE)".into(),
        Some(index),
        3,
        datum,
    )];

    let found = mask_found(found, Some(&mask()));

    let masked = token(Phi::Name, datum);
    assert_eq!(found[0].datum, masked.as_bytes());
    assert_eq!(
        message(&found[0]),
        format!("contains an invalid character({masked})")
    );
}

#[test]
fn mask_found_masks_flagged_values_a_segment_level_message_quotes() {
    let (index, id, text) = first_claim();
    let found = vec![segment_level(
        format!("Segment CLP*{text} not found."),
        index,
        b"CLP",
    )];

    let found = mask_found(found, Some(&mask()));

    assert_eq!(found[0].datum, b"CLP".to_vec());
    assert_eq!(
        message(&found[0]),
        format!("Segment CLP*{} not found.", token(Phi::Id, &id))
    );
}

#[test]
fn mask_found_rewrites_a_message_once_when_the_datum_and_the_segment_quote_values() {
    let (index, id, text) = first_claim();
    let found = vec![external(
        format!("1 is not CLP*{text}"),
        Some(index),
        1,
        b"1",
    )];

    let found = mask_found(found, Some(&mask()));

    assert_eq!(
        message(&found[0]),
        format!(
            "{} is not CLP*{}",
            token(Phi::Id, b"1"),
            token(Phi::Id, &id)
        )
    );
}

#[test]
fn mask_found_leaves_values_of_another_segment_in_the_message_as_written() {
    let (_, _, text) = first_claim();
    let (index, _) = find(b"TRN", None);
    let original = segment_level(format!("Segment CLP*{text} not found."), index, b"TRN");

    assert_unchanged(vec![original], Some(&mask()));
}

/// An original and the masked form `unquoted` writes for it.
type Pair<'a> = (&'a [u8], &'a str);

/// The originals and masked forms `unquoted` takes, from text pairs.
fn pairs(values: &[Pair<'_>]) -> Vec<(Vec<u8>, Vec<u8>)> {
    values
        .iter()
        .map(|(original, masked)| (original.to_vec(), masked.as_bytes().to_vec()))
        .collect()
}

#[test]
fn mask_found_masks_a_one_character_datum_quoted_after_a_word_that_starts_with_it() {
    let (index, _) = find(b"NM1", Some(b"QC"));
    let found = vec![external(
        "Data element too long: \"D\"".into(),
        Some(index),
        3,
        b"D",
    )];

    let found = mask_found(found, Some(&mask()));

    assert_eq!(
        message(&found[0]),
        format!("Data element too long: \"{}\"", token(Phi::Name, b"D"))
    );
}

#[test]
fn unquoted_replaces_a_short_value_that_stands_alone() {
    let quoted = pairs(&[(b"7", "<7>")]);
    for (message, expected) in [
        ("Segment CLP*7*1 not found", "Segment CLP*<7>*1 not found"),
        ("value (7)", "value (<7>)"),
        ("value '7'", "value '<7>'"),
        ("value \"7\"", "value \"<7>\""),
        ("7", "<7>"),
        ("7 is first", "<7> is first"),
        ("last is 7", "last is <7>"),
        ("7*7", "<7>*<7>"),
    ] {
        assert_eq!(unquoted(message, &quoted), expected, "{message}");
    }
}

#[test]
fn unquoted_leaves_a_short_value_inside_a_word_or_number() {
    let quoted = pairs(&[(b"7", "<7>"), (b"D", "<D>")]);
    for message in ["CLP07", "17 of 27", "7A", "Data element", "ID", "x7y"] {
        assert_eq!(unquoted(message, &quoted), message);
    }
}

#[test]
fn unquoted_replaces_a_value_of_four_characters_or_more_anywhere() {
    let quoted = pairs(&[(b"ABCD", "<abcd>")]);
    assert_eq!(unquoted("xABCDx and ABCD", &quoted), "x<abcd>x and <abcd>");
}

#[test]
fn unquoted_replaces_overlapping_values_by_their_tokens_in_start_order() {
    for quoted in [
        pairs(&[(b"XAB", "<x>"), (b"ABYZ", "<y>")]),
        pairs(&[(b"ABYZ", "<y>"), (b"XAB", "<x>")]),
    ] {
        assert_eq!(unquoted("(XABYZ)", &quoted), "(<x><y>)");
        assert_eq!(unquoted("XABYZ", &quoted), "<x><y>");
    }
    let quoted = pairs(&[(b"WXAB", "<w>"), (b"ABYZ", "<y>")]);
    assert_eq!(unquoted("is WXABYZ.", &quoted), "is <w><y>.");
}

#[test]
fn unquoted_replaces_a_value_held_in_a_longer_one_by_the_longer_token() {
    let quoted = pairs(&[(b"AB", "<ab>"), (b"ABCD", "<abcd>")]);
    assert_eq!(unquoted("ABCD AB", &quoted), "<abcd> <ab>");
}

#[test]
fn unquoted_reads_a_short_value_as_latin_1_or_utf_8_on_boundaries() {
    let latin1 = pairs(&[(b"D\xe9", "<d>")]);
    assert_eq!(
        unquoted("character(D\u{e9}) not XD\u{e9}", &latin1),
        "character(<d>) not XD\u{e9}"
    );
    let utf8 = pairs(&[("\u{e9}".as_bytes(), "<e>")]);
    assert_eq!(
        unquoted("character(\u{e9}) not a\u{e9}", &utf8),
        "character(<e>) not a\u{e9}"
    );
}

#[test]
fn unquoted_replaces_short_values_glued_to_each_other_as_one_group() {
    let cases: [(&[Pair<'_>], &str, &str); 4] = [
        (&[(b"AB", "<ab>"), (b"CD", "<cd>")], "ABCD", "<ab><cd>"),
        (&[(b"D", "<d>"), (b"J", "<j>")], "*DJ*", "*<d><j>*"),
        (&[(b"AB", "<ab>"), (b"BC", "<bc>")], "(ABC)", "(<ab><bc>)"),
        (
            &[(b"XXXX", "<l>"), (b"AB", "<ab>"), (b"BC", "<bc>")],
            "XXXXABC",
            "<l><ab><bc>",
        ),
    ];
    for (values, message, expected) in cases {
        assert_eq!(unquoted(message, &pairs(values)), expected, "{message}");
    }
}

#[test]
fn unquoted_leaves_a_group_of_short_values_glued_to_a_word() {
    let quoted = pairs(&[(b"AB", "<ab>"), (b"CD", "<cd>")]);
    assert_eq!(unquoted("xABCD", &quoted), "xABCD");
}

#[test]
fn unquoted_leaves_a_short_value_glued_to_a_letter_no_value_holds() {
    let quoted = pairs(&[(b"XAB", "<x>"), (b"ABYZ", "<y>")]);
    assert_eq!(unquoted("QXABYZ", &quoted), "QX<y>");
}

#[test]
fn unquoted_replaces_a_word_two_short_values_spell_together() {
    let quoted = pairs(&[(b"A", "<a>"), (b"N", "<n>")]);
    assert_eq!(
        unquoted("Data element is type AN", &quoted),
        "Data element is type <a><n>"
    );
}

#[test]
fn unquoted_counts_a_value_by_the_characters_of_its_utf_8_reading() {
    let quoted = pairs(&[("AB\u{e9}".as_bytes(), "<v>")]);
    assert_eq!(
        unquoted("xAB\u{c3}\u{a9}x and xAB\u{e9}x", &quoted),
        "xAB\u{c3}\u{a9}x and xAB\u{e9}x"
    );
    assert_eq!(
        unquoted("(AB\u{c3}\u{a9}) and (AB\u{e9})", &quoted),
        "(<v>) and (<v>)"
    );
}

#[test]
fn unquoted_takes_an_accented_letter_or_a_hyphen_as_a_boundary() {
    let quoted = pairs(&[(b"D", "<d>")]);
    assert_eq!(unquoted("D\u{e9} and D-1", &quoted), "<d>\u{e9} and <d>-1");
}

#[test]
fn a_date_with_no_year_start_is_masked_as_keyed_text() {
    let mask = mask();
    let token = mask.text(Phi::Date, b"20240318").to_vec();
    assert_eq!(date_datum(&mask, Some(i32::MIN), b"20240318"), token);
    assert_eq!(date_datum(&mask, None, b"20240318"), token);
    assert_eq!(date_datum(&mask, Some(19_800), b"20240318"), b"20240101");
    assert_eq!(masked_date(DateMode::Year, i32::MIN, None), None);
    assert_eq!(masked_date(DateMode::Shift, i32::MIN, Some(-1)), None);
    assert_eq!(masked_date(DateMode::Year, 19_800, None), Some(19_723));
}
