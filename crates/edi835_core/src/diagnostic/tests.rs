use super::*;
use crate::Delimiters;

fn path(loops: &[(&str, usize)]) -> Vec<LoopRef> {
    loops
        .iter()
        .map(|&(name, ordinal)| LoopRef {
            name: name.to_string(),
            ordinal,
        })
        .collect()
}

const TRANSACTION: &[(&str, usize)] = &[("interchange", 1), ("group", 1), ("transaction", 1)];

#[test]
fn every_rule_names_its_variant() {
    let id = || b"SE".to_vec();
    let rules = [
        (Rule::ByteOrderMark, "ByteOrderMark"),
        (Rule::UnknownSegment { id: id() }, "UnknownSegment"),
        (
            Rule::ImplicitLoop {
                loop_name: "group".into(),
                expected_trigger: "\"GS\" with no conditions".into(),
                caused_by: b"ST".to_vec(),
            },
            "ImplicitLoop",
        ),
        (
            Rule::UnterminatedLoop {
                loop_name: "transaction".into(),
                expected_end: id(),
                opened_at: None,
            },
            "UnterminatedLoop",
        ),
        (
            Rule::ControlCountMismatch {
                segment_id: id(),
                element: 1,
                expected: 2,
                found: b"3".to_vec(),
            },
            "ControlCountMismatch",
        ),
        (
            Rule::ControlElementMissing {
                segment_id: id(),
                element: 1,
            },
            "ControlElementMissing",
        ),
        (
            Rule::ControlNumberMismatch {
                opener: b"ST".to_vec(),
                opener_element: 2,
                closer: id(),
                closer_element: 2,
                opener_value: b"1".to_vec(),
                closer_value: b"2".to_vec(),
                opened_at: Some(0),
            },
            "ControlNumberMismatch",
        ),
        (
            Rule::RequiredElementMissing {
                segment_id: id(),
                element: 1,
                component: None,
                name: "n".into(),
            },
            "RequiredElementMissing",
        ),
        (
            Rule::TypeMismatch {
                segment_id: id(),
                element: 1,
                component: None,
                name: "n".into(),
                expected: ElementType::N(0),
            },
            "TypeMismatch",
        ),
        (
            Rule::LengthOutOfRange {
                segment_id: id(),
                element: 1,
                component: None,
                name: "n".into(),
                min: Some(1),
                max: Some(2),
                length: 3,
            },
            "LengthOutOfRange",
        ),
        (
            Rule::ValueDropped {
                table: "t".into(),
                column: "c".into(),
                bytes: 1,
            },
            "ValueDropped",
        ),
        (
            Rule::CompositeShape {
                segment_id: id(),
                element: 1,
                name: "n".into(),
                declared: 1,
                found: 2,
            },
            "CompositeShape",
        ),
        (
            Rule::CodeNotInList {
                segment_id: id(),
                element: 1,
                component: None,
                name: "n".into(),
                codes: 2,
            },
            "CodeNotInList",
        ),
    ];
    for (rule, kind) in rules {
        assert_eq!(rule.kind(), kind);
        assert!(format!("{rule:?}").starts_with(kind), "{rule:?}");
    }
}

#[test]
fn levels_display_as_snip_numbers() {
    assert_eq!(SnipLevel::L1.to_string(), "SNIP 1");
    assert_eq!(SnipLevel::L2.to_string(), "SNIP 2");
    assert_eq!(SnipLevel::L3.to_string(), "SNIP 3");
}

#[test]
fn a_loop_ref_displays_name_and_ordinal() {
    let at = LoopRef {
        name: "2100".into(),
        ordinal: 3,
    };
    assert_eq!(at.to_string(), "2100#3");
}

#[test]
fn a_loop_ref_quotes_a_name_that_holds_a_separator() {
    let at = LoopRef {
        name: "x/y".into(),
        ordinal: 2,
    };
    assert_eq!(at.to_string(), "\"x/y\"#2");
    let at = LoopRef {
        name: "x#2".into(),
        ordinal: 1,
    };
    assert_eq!(at.to_string(), "\"x#2\"#1");
}

#[test]
fn byte_order_mark_displays_the_first_segment_and_the_mark() {
    let diagnostic = Diagnostic::new(
        Rule::ByteOrderMark,
        Some(0),
        None,
        None,
        Vec::new(),
        b"\xEF\xBB\xBF".to_vec(),
    );
    assert_eq!(diagnostic.level, SnipLevel::L1);
    assert_eq!(
        diagnostic.to_string(),
        "SNIP 1 · the input starts with a UTF-8 byte order mark, kept as leading trivia of the first segment · segment #0 · at the root · datum \"\\u{feff}\""
    );
}

#[test]
fn unknown_segment_displays_id_index_path_and_datum() {
    let diagnostic = Diagnostic::new(
        Rule::UnknownSegment { id: b"XX".to_vec() },
        Some(7),
        None,
        None,
        path(&[
            ("interchange", 1),
            ("group", 1),
            ("transaction", 1),
            ("1000A", 1),
        ]),
        b"XX".to_vec(),
    );
    assert_eq!(diagnostic.level, SnipLevel::L1);
    assert_eq!(
        diagnostic.to_string(),
        "SNIP 1 · segment \"XX\" is not part of the structure: no open loop holds it and it opens no loop · segment #7 · at interchange#1/group#1/transaction#1/1000A#1 · datum \"XX\""
    );
}

#[test]
fn implicit_loop_displays_the_loop_and_the_segment_that_needed_it() {
    let diagnostic = Diagnostic::new(
        Rule::ImplicitLoop {
            loop_name: "group".into(),
            expected_trigger: "\"GS\" with no conditions".into(),
            caused_by: b"ST".to_vec(),
        },
        Some(0),
        None,
        None,
        path(&[("interchange", 1), ("group", 1)]),
        b"ST".to_vec(),
    );
    assert_eq!(
        diagnostic.to_string(),
        "SNIP 1 · loop \"group\" opened without its own trigger (\"GS\" with no conditions) to hold segment \"ST\" · segment #0 · at interchange#1/group#1 · datum \"ST\""
    );
}

#[test]
fn unterminated_loop_displays_the_opener_the_expected_end_and_the_closing_segment() {
    let diagnostic = Diagnostic::new(
        Rule::UnterminatedLoop {
            loop_name: "transaction".into(),
            expected_end: b"SE".to_vec(),
            opened_at: Some(2),
        },
        Some(4),
        None,
        None,
        path(TRANSACTION),
        b"GE".to_vec(),
    );
    assert_eq!(
        diagnostic.to_string(),
        "SNIP 1 · loop \"transaction\" opened at segment #2 closed without its end segment \"SE\" · segment #4 · at interchange#1/group#1/transaction#1 · datum \"GE\""
    );
    let implicit = Rule::UnterminatedLoop {
        loop_name: "transaction".into(),
        expected_end: b"SE".to_vec(),
        opened_at: None,
    };
    assert_eq!(
        implicit.to_string(),
        "loop \"transaction\" closed without its end segment \"SE\""
    );
}

#[test]
fn a_finding_at_the_end_of_the_stream_says_so() {
    let diagnostic = Diagnostic::new(
        Rule::UnterminatedLoop {
            loop_name: "interchange".into(),
            expected_end: b"IEA".to_vec(),
            opened_at: Some(0),
        },
        None,
        None,
        None,
        path(&[("interchange", 1)]),
        Vec::new(),
    );
    assert_eq!(
        diagnostic.to_string(),
        "SNIP 1 · loop \"interchange\" opened at segment #0 closed without its end segment \"IEA\" · end of stream · at interchange#1 · datum \"\""
    );
}

#[test]
fn control_count_mismatch_displays_the_element_the_value_and_the_count() {
    let diagnostic = Diagnostic::new(
        Rule::ControlCountMismatch {
            segment_id: b"SE".to_vec(),
            element: 1,
            expected: 18,
            found: b"15".to_vec(),
        },
        Some(19),
        Some(1),
        None,
        path(TRANSACTION),
        b"15".to_vec(),
    );
    assert_eq!(
        diagnostic.to_string(),
        "SNIP 1 · SE01 declares \"15\" but the count is 18 · segment #19, element 1 · at interchange#1/group#1/transaction#1 · datum \"15\""
    );
}

#[test]
fn control_element_missing_displays_the_segment_and_the_position() {
    let diagnostic = Diagnostic::new(
        Rule::ControlElementMissing {
            segment_id: b"SE".to_vec(),
            element: 2,
        },
        Some(4),
        Some(2),
        None,
        path(TRANSACTION),
        Vec::new(),
    );
    assert_eq!(
        diagnostic.to_string(),
        "SNIP 1 · control element SE02 is missing: the segment has no element 2 · segment #4, element 2 · at interchange#1/group#1/transaction#1 · datum \"\""
    );
}

#[test]
fn control_number_mismatch_displays_both_elements_values_and_the_opener() {
    let diagnostic = Diagnostic::new(
        Rule::ControlNumberMismatch {
            opener: b"ST".to_vec(),
            opener_element: 2,
            closer: b"SE".to_vec(),
            closer_element: 2,
            opener_value: b"0001".to_vec(),
            closer_value: b"0002".to_vec(),
            opened_at: Some(2),
        },
        Some(4),
        Some(2),
        None,
        path(TRANSACTION),
        b"0002".to_vec(),
    );
    assert_eq!(
        diagnostic.to_string(),
        "SNIP 1 · SE02 \"0002\" does not match ST02 \"0001\" of segment #2 · segment #4, element 2 · at interchange#1/group#1/transaction#1 · datum \"0002\""
    );
    let implicit = Rule::ControlNumberMismatch {
        opener: b"ST".to_vec(),
        opener_element: 2,
        closer: b"SE".to_vec(),
        closer_element: 2,
        opener_value: b"0001".to_vec(),
        closer_value: b"0002".to_vec(),
        opened_at: None,
    };
    assert_eq!(
        implicit.to_string(),
        "SE02 \"0002\" does not match ST02 \"0001\""
    );
}

#[test]
fn required_element_missing_displays_the_element_and_its_name() {
    let diagnostic = Diagnostic::new(
        Rule::RequiredElementMissing {
            segment_id: b"CLP".to_vec(),
            element: 1,
            component: None,
            name: "claim_submitter_identifier".into(),
        },
        Some(12),
        Some(1),
        None,
        path(&[("transaction", 1), ("2000", 1), ("2100", 1)]),
        Vec::new(),
    );
    assert_eq!(
        diagnostic.level.to_string(),
        "SNIP 2",
        "element rules are level 2"
    );
    assert_eq!(
        diagnostic.to_string(),
        "SNIP 2 · required element CLP01 (claim_submitter_identifier) is missing or empty · segment #12, element 1 · at transaction#1/2000#1/2100#1 · datum \"\""
    );
}

#[test]
fn type_mismatch_displays_the_component_and_the_declared_type() {
    let diagnostic = Diagnostic::new(
        Rule::TypeMismatch {
            segment_id: b"CLP".to_vec(),
            element: 3,
            component: None,
            name: "total_claim_charge_amount".into(),
            expected: ElementType::R { scale: 2 },
        },
        Some(12),
        Some(3),
        None,
        path(&[("2100", 2)]),
        b"12A".to_vec(),
    );
    assert_eq!(
        diagnostic.to_string(),
        "SNIP 2 · element CLP03 (total_claim_charge_amount) is not a valid R (decimal, scale 2) · segment #12, element 3 · at 2100#2 · datum \"12A\""
    );
    let component = Rule::TypeMismatch {
        segment_id: b"SVC".to_vec(),
        element: 1,
        component: Some(1),
        name: "product_or_service_id_qualifier".into(),
        expected: ElementType::Id,
    };
    assert_eq!(
        component.to_string(),
        "element SVC01-1 (product_or_service_id_qualifier) is not a valid ID (code)"
    );
}

#[test]
fn length_out_of_range_displays_the_length_and_the_bounds() {
    let rule = |min, max| Rule::LengthOutOfRange {
        segment_id: b"CLP".to_vec(),
        element: 1,
        component: None,
        name: "claim_submitter_identifier".into(),
        min,
        max,
        length: 40,
    };
    let diagnostic = Diagnostic::new(
        rule(Some(1), Some(38)),
        Some(12),
        Some(1),
        None,
        Vec::new(),
        b"0123456789012345678901234567890123456789".to_vec(),
    );
    assert_eq!(
        diagnostic.to_string(),
        "SNIP 2 · element CLP01 (claim_submitter_identifier) has length 40; the spec allows 1 to 38 · segment #12, element 1 · at the root · datum \"0123456789012345678901234567890123456789\""
    );
    assert!(
        rule(Some(41), None)
            .to_string()
            .ends_with("allows at least 41")
    );
    assert!(
        rule(None, Some(38))
            .to_string()
            .ends_with("allows at most 38")
    );
    assert!(rule(None, None).to_string().ends_with("allows any length"));
}

#[test]
fn code_not_in_list_displays_the_element_the_list_size_and_the_value() {
    let rule = |component, codes| Rule::CodeNotInList {
        segment_id: b"SVC".to_vec(),
        element: 1,
        component,
        name: "product_or_service_id_qualifier".into(),
        codes,
    };
    let diagnostic = Diagnostic::new(
        rule(Some(1), 10),
        Some(17),
        Some(1),
        Some(1),
        path(&[("2110", 3)]),
        b"ZZ".to_vec(),
    );
    assert_eq!(diagnostic.level, SnipLevel::L2);
    assert_eq!(
        diagnostic.to_string(),
        "SNIP 2 · element SVC01-1 (product_or_service_id_qualifier) is not one of the 10 codes the spec lists for it · segment #17, element 1, component 1 · at 2110#3 · datum \"ZZ\""
    );
    assert_eq!(
        rule(None, 1).to_string(),
        "element SVC01 (product_or_service_id_qualifier) is not the one code the spec lists for it"
    );
}

#[test]
fn composite_shape_displays_found_and_declared_components() {
    let diagnostic = Diagnostic::new(
        Rule::CompositeShape {
            segment_id: b"SVC".to_vec(),
            element: 1,
            name: "composite_medical_procedure".into(),
            declared: 8,
            found: 9,
        },
        Some(17),
        Some(1),
        Some(9),
        path(&[("2110", 1)]),
        b"X".to_vec(),
    );
    assert_eq!(
        diagnostic.to_string(),
        "SNIP 2 · element SVC01 (composite_medical_procedure) has 9 components; the spec declares 8 · segment #17, element 1, component 9 · at 2110#1 · datum \"X\""
    );
}

#[test]
fn value_dropped_displays_the_table_the_column_the_byte_total_and_the_value() {
    let diagnostic = Diagnostic::new(
        Rule::ValueDropped {
            table: "claims".into(),
            column: "note".into(),
            bytes: 2147483650,
        },
        Some(9),
        None,
        None,
        path(&[("2100", 4)]),
        b"a long note".to_vec(),
    );
    assert_eq!(
        diagnostic.level.to_string(),
        "SNIP 2",
        "a dropped value is a level 2 finding"
    );
    assert_eq!(
        diagnostic.to_string(),
        "SNIP 2 · text for column \"note\" of table \"claims\" was not stored: it would bring the column to 2147483650 bytes and a column holds at most 2147483647 · segment #9 · at 2100#4 · datum \"a long note\""
    );
}

#[test]
fn invalid_utf8_in_a_datum_is_shown_as_hex_escapes() {
    let diagnostic = Diagnostic::new(
        Rule::UnknownSegment {
            id: vec![b'Z', 0xFF],
        },
        Some(1),
        None,
        None,
        Vec::new(),
        vec![b'Z', 0xFF],
    );
    assert_eq!(
        diagnostic.to_string(),
        "SNIP 1 · segment \"Z\\xFF\" is not part of the structure: no open loop holds it and it opens no loop · segment #1 · at the root · datum \"Z\\xFF\""
    );
    let mixed = Quoted(b"a\"b\n\xC3\xA9\xE9\x80c");
    assert_eq!(mixed.to_string(), "\"a\\\"b\\n\u{e9}\\xE9\\x80c\"");
}

#[test]
fn span_resolves_the_segment_bytes_from_the_document() {
    let document =
        Document::with_delimiters(&b"AA*1~BB*2~"[..], Delimiters::new(b'*', b':', b'~')).unwrap();
    let at = |segment| {
        Diagnostic::new(
            Rule::UnknownSegment { id: b"BB".to_vec() },
            segment,
            None,
            None,
            Vec::new(),
            b"BB".to_vec(),
        )
    };
    let span = at(Some(1)).span(&document).unwrap();
    assert_eq!(&document.as_bytes()[span.raw], b"BB*2~");
    assert_eq!(at(Some(9)).span(&document), None);
    assert_eq!(at(None).span(&document), None);
}
