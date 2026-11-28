use super::super::envelope::interchange_version;
use super::super::envelope::trigger_rule;
use super::super::{
    Envelope, EnvelopeValueError, Finding, GroupCode, Refusal, WriteError, write,
    write_with_findings,
};
use crate::delimiters::Delimiters;
use crate::diagnostic::Rule;
use crate::spec::test_support::{builtin, patched};
use crate::spec_load::Version;
use crate::value::TimeUnit;
use crate::{Document, Processor, Spec};

#[test]
fn with_ids_uses_the_default_qualifiers() {
    let envelope = Envelope::with_ids("SENDER", "RECEIVER", 19_782, 3_600);
    assert_eq!(envelope.sender_qualifier, "ZZ");
    assert_eq!(envelope.receiver_qualifier, "ZZ");
    assert_eq!(envelope.sender_id, "SENDER");
    assert_eq!(envelope.receiver_id, "RECEIVER");
    assert_eq!(envelope.usage_indicator, "P");
    assert_eq!(envelope.control_number, 1);
    assert_eq!((envelope.date, envelope.time), (19_782, 3_600));
}

#[test]
fn the_defaults_are_the_constants() {
    assert_eq!(Envelope::DEFAULT_QUALIFIER, "ZZ");
    assert_eq!(Envelope::DEFAULT_USAGE_INDICATOR, "P");
    assert_eq!(Envelope::DEFAULT_CONTROL_NUMBER, 1);
}

const BALANCED_5010: &[u8] = include_bytes!("../../../tests/fixtures/balanced_5010_sample.txt");
const BALANCED_4010: &[u8] = include_bytes!("../../../tests/fixtures/balanced_4010_sample.txt");

fn envelope_with(delimiters: Delimiters) -> Envelope {
    let mut envelope = Envelope::with_ids("SENDER", "RECEIVER", 19_723, 45_000);
    envelope.delimiters = delimiters;
    envelope
}

/// The tables `spec` reads from `bytes`, written with `envelope`.
fn written(spec: &Spec, bytes: &[u8], envelope: &Envelope) -> Result<Vec<u8>, WriteError> {
    let document = Document::parse(bytes).unwrap();
    let (tables, _) = Processor::run(spec, &document, None);
    write(spec, &tables, envelope)
}

#[test]
fn a_00402_spec_with_a_code_on_isa11_writes_the_separator() {
    let spec = patched(
        &builtin(Version::V5010),
        r#"{"segments":{"ISA":{"elements":{"11":{"codes":["U"]}}}}}"#,
    )
    .unwrap();
    let envelope = envelope_with(Delimiters::new(b'*', b':', b'~').with_repetition(b'^'));
    let document = Document::parse(BALANCED_5010).unwrap();
    let (tables, _) = Processor::run(&spec, &document, None);
    let (bytes, findings) = write_with_findings(&spec, &tables, &envelope).unwrap();
    // The spec's code list for ISA11 does not hold the separator, so the
    // read-back reports it, and only that.
    assert!(
        matches!(
            findings.as_slice(),
            [Finding::ReadBack { diagnostic, .. }]
                if matches!(diagnostic.rule, Rule::CodeNotInList { element: 11, .. })
                    && diagnostic.element == Some(11)
        ),
        "{findings:#?}"
    );
    let isa = String::from_utf8_lossy(&bytes);
    assert!(isa.contains("*^*00501*"), "{isa}");
    assert_eq!(Delimiters::from_isa(&bytes).unwrap().repetition, Some(b'^'));
}

#[test]
fn a_spec_below_00402_without_a_code_needs_no_separator() {
    let spec = patched(
        &builtin(Version::V4010),
        r#"{"segments":{"ISA":{"elements":{"11":{"codes":null,"usage":"situational"}}}}}"#,
    )
    .unwrap();
    let envelope = envelope_with(Delimiters::new(b'*', b':', b'~'));
    let bytes = written(&spec, BALANCED_4010, &envelope).unwrap();
    assert_eq!(Delimiters::from_isa(&bytes).unwrap().repetition, None);
}

#[test]
fn a_00402_spec_without_a_separator_is_refused() {
    let spec = Spec::builtin_835();
    let envelope = envelope_with(Delimiters::new(b'*', b':', b'~'));
    assert!(matches!(
        written(&spec, BALANCED_5010, &envelope),
        Err(WriteError::NoRepetition { version }) if version == b"00501"
    ));
}

#[test]
fn a_spec_with_no_value_for_isa12_writes_no_separator() {
    let spec = patched(&builtin(Version::V5010),
            r#"{"segments":{"ISA":{"elements":{"11":{"usage":"situational"},"12":{"codes":null,"usage":"situational"}}}}}"#,
        )
        .unwrap();
    let envelope = envelope_with(Delimiters::new(b'*', b':', b'~').with_repetition(b'^'));
    let bytes = written(&spec, BALANCED_5010, &envelope).unwrap();
    assert_eq!(Delimiters::from_isa(&bytes).unwrap().repetition, None);
}

#[test]
fn a_declared_version_that_is_not_isa12_does_not_decide_the_separator() {
    let spec = patched(&builtin(Version::V4010),
            r#"{"version":{"values":["4010"]},"segments":{"GS":{"elements":{"8":{"codes":["4010"]}}}}}"#,
        )
        .unwrap();
    let envelope = envelope_with(Delimiters::new(b'*', b':', b'~'));
    let bytes = written(&spec, BALANCED_4010, &envelope).unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("*U*00401*"));
    assert_eq!(Delimiters::from_isa(&bytes).unwrap().repetition, None);
}

#[test]
fn a_below_00402_spec_with_a_code_writes_the_code_over_the_envelope_separator() {
    let envelope = envelope_with(Delimiters::new(b'*', b':', b'~').with_repetition(b'^'));
    let bytes = written(&Spec::builtin_835_4010(), BALANCED_4010, &envelope).unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("*U*00401*"));
}

/// Element `element` of the first `segment` in `bytes`, with `~` and `*`.
fn written_element(bytes: &[u8], segment: &[u8], element: usize) -> Vec<u8> {
    let found = bytes
        .split(|&byte| byte == b'~')
        .find(|text| text.split(|&byte| byte == b'*').next() == Some(segment))
        .unwrap();
    found
        .split(|&byte| byte == b'*')
        .nth(element)
        .unwrap()
        .to_vec()
}

#[test]
fn the_interchange_version_is_the_value_the_header_writes() {
    let only_a_code = patched(
        &builtin(Version::V5010),
        r#"{"segments":{"ISA":{"elements":{"12":{"codes":["00402"]}}}}}"#,
    )
    .unwrap();
    let declared = patched(&builtin(Version::V5010),
            r#"{"version":{"segment":"ISA","element":12,"values":["00401"]},
                "segments":{"ISA":{"elements":{"11":{"usage":"situational"},"12":{"codes":["00401"]}}}}}"#,
        )
        .unwrap();
    let neither = patched(&builtin(Version::V5010),
            r#"{"segments":{"ISA":{"elements":{"11":{"usage":"situational"},"12":{"codes":null,"usage":"situational"}}}}}"#,
        )
        .unwrap();
    let envelope = envelope_with(Delimiters::new(b'*', b':', b'~').with_repetition(b'^'));
    let cases: [(Spec, &[u8], &[u8]); 5] = [
        (Spec::builtin_835(), BALANCED_5010, b"00501"),
        (Spec::builtin_835_4010(), BALANCED_4010, b"00401"),
        (only_a_code, BALANCED_5010, b"00402"),
        (declared, BALANCED_5010, b"00401"),
        (neither, BALANCED_5010, b"     "),
    ];
    for (spec, input, expected) in cases {
        assert_eq!(interchange_version(&spec, &envelope), expected);
        let bytes = written(&spec, input, &envelope).unwrap();
        assert_eq!(written_element(&bytes, b"ISA", 12), expected);
    }
}

fn plain_envelope() -> Envelope {
    envelope_with(Delimiters::new(b'*', b':', b'~').with_repetition(b'^'))
}

#[test]
fn a_narrowed_trigger_occurrence_code_is_written() {
    let spec = patched(
        &builtin(Version::V5010),
        r#"{"loops":{"interchange":{"occurrences":{"interchange_control_header":{"elements":{"14":{"codes":["1"]}}}}}}}"#,
    )
    .unwrap();
    let bytes = written(&spec, BALANCED_5010, &plain_envelope()).unwrap();
    assert_eq!(written_element(&bytes, b"ISA", 14), b"1");
}

#[test]
fn the_trigger_occurrence_sets_the_written_widths() {
    let cases: [(&str, &[u8], usize, &[u8]); 3] = [
        (
            r#"{"loops":{"group":{"occurrences":{"functional_group_header":{"elements":{"2":{"min":10,"max":10}}}}}}}"#,
            b"GS",
            2,
            b"SENDER    ",
        ),
        (
            r#"{"segments":{"GS":{"elements":{"4":{"min":6}}}},"loops":{"group":{"occurrences":{"functional_group_header":{"elements":{"4":{"max":6}}}}}}}"#,
            b"GS",
            4,
            b"240101",
        ),
        (
            r#"{"loops":{"transaction":{"occurrences":{"transaction_set_header":{"elements":{"2":{"min":6}}}}}}}"#,
            b"ST",
            2,
            b"000001",
        ),
    ];
    for (patch, segment, element, expected) in cases {
        let spec = patched(&builtin(Version::V5010), patch).unwrap();
        let (bytes, _) = written_with_findings(&spec);
        assert_eq!(written_element(&bytes, segment, element), expected);
        if segment == b"ST" {
            assert_eq!(written_element(&bytes, b"SE", 2), expected);
        }
    }
}

#[test]
fn a_loop_without_occurrences_reads_the_definition() {
    let loaded = builtin(Version::V5010);
    let spec = patched(
        &loaded,
        r#"{"loops":{"interchange":{"occurrences":{"interchange_control_header":null,"interchange_acknowledgement":null}}}}"#,
    )
    .unwrap();
    let root = spec.get(spec.roots()[0]);
    assert!(root.occurrences.is_empty());
    let rule = trigger_rule(&spec, root, 14).unwrap();
    assert_eq!(rule.codes, ["0", "1"]);
    let rule = trigger_rule(&spec, root, 6).unwrap();
    assert_eq!(rule.length.min, Some(15));
}

#[test]
fn an_envelope_loop_without_occurrences_is_refused_by_the_writer() {
    let spec = patched(
        &builtin(Version::V5010),
        r#"{"loops":{"interchange":{"occurrences":{"interchange_control_header":null,"interchange_acknowledgement":null}}}}"#,
    )
    .unwrap();
    let Err(WriteError::Plan(plan)) = write(&spec, &Default::default(), &plain_envelope()) else {
        panic!("a spec with no ISA occurrence must be refused");
    };
    assert!(plan.refusals.contains(&Refusal::EnvelopeWithoutOccurrence {
        loop_name: "interchange".into(),
        trigger: super::bare(b"ISA"),
    }));
}

#[test]
fn every_builtin_envelope_trigger_reads_its_definition_at_the_trigger_occurrence() {
    for version in [Version::V5010, Version::V4010] {
        let loaded = builtin(version);
        let spec = &*loaded.spec;
        for def in spec.loops().iter().filter(|def| def.control.is_some()) {
            let segment = spec.segment(&def.trigger.segment).unwrap();
            for (&position, element) in &segment.elements {
                let rule = trigger_rule(spec, def, position).unwrap();
                assert_eq!(rule.codes, element.codes.as_slice(), "{:?}", def.trigger);
                assert_eq!(rule.length, element.length, "{:?}", def.trigger);
                assert_eq!(rule.required, element.required, "{:?}", def.trigger);
            }
        }
    }
}

/// `BALANCED_5010` written by `spec` with the plain envelope, with the findings.
fn written_with_findings(spec: &Spec) -> (Vec<u8>, Vec<Finding>) {
    let document = Document::parse(BALANCED_5010).unwrap();
    let (tables, _) = Processor::run(spec, &document, None);
    write_with_findings(spec, &tables, &plain_envelope()).unwrap()
}

#[test]
fn a_caller_field_moved_to_another_element_is_written_there() {
    let spec = patched(
        &builtin(Version::V5010),
        r#"{"loops":{"group":{"control":{"fields":{"2":"application_receiver","3":"application_sender"}}}}}"#,
    )
    .unwrap();
    let bytes = written(&spec, BALANCED_5010, &plain_envelope()).unwrap();
    assert_eq!(written_element(&bytes, b"GS", 2), b"RECEIVER");
    assert_eq!(written_element(&bytes, b"GS", 3), b"SENDER");
}

#[test]
fn a_caller_field_on_a_place_the_syntax_layer_owns_is_refused() {
    for element in ["11", "12", "16"] {
        let patch = format!(
            r#"{{"loops":{{"interchange":{{"control":{{"fields":{{"10":null,"{element}":"time"}}}}}}}}}}"#
        );
        let err = patched(&builtin(Version::V5010), &patch)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains(&format!("\"time\" on element {element} of the top")),
            "{err}"
        );
    }
}

#[test]
fn a_spec_whose_envelope_states_no_fields_reports_the_elements_left_empty() {
    let spec = patched(
        &builtin(Version::V5010),
        r#"{"loops":{"interchange":{"control":{"fields":null}},"group":{"control":{"fields":null}}}}"#,
    )
    .unwrap();
    let (bytes, findings) = written_with_findings(&spec);
    assert_eq!(written_element(&bytes, b"GS", 2), b"");
    assert_eq!(written_element(&bytes, b"ISA", 6), b"               ");
    let missing = |element: usize| {
        findings.iter().any(|finding| {
            matches!(
                finding,
                Finding::ReadBack { diagnostic, .. }
                    if diagnostic.segment == Some(1)
                        && diagnostic.element == Some(element)
                        && matches!(diagnostic.rule, Rule::RequiredElementMissing { .. })
            )
        })
    };
    assert!(missing(2), "{findings:?}");
    assert!(missing(3), "{findings:?}");
}

#[test]
fn a_group_code_is_the_interchange_id_unless_given() {
    let spec = builtin(Version::V5010).spec;
    let mut envelope = plain_envelope();
    let bytes = written(&spec, BALANCED_5010, &envelope).unwrap();
    assert_eq!(written_element(&bytes, b"GS", 2), b"SENDER");
    envelope.application_sender = GroupCode::Given("APP".into());
    let bytes = written(&spec, BALANCED_5010, &envelope).unwrap();
    assert_eq!(written_element(&bytes, b"GS", 2), b"APP");
    assert_eq!(GroupCode::default(), GroupCode::InterchangeId);
    let mut padded = plain_envelope();
    padded.receiver_id = "RECEIVER   ".into();
    padded.application_receiver = GroupCode::Given("GRP".into());
    let bytes = written(&spec, BALANCED_5010, &padded).unwrap();
    assert_eq!(written_element(&bytes, b"GS", 3), b"GRP");
    padded.application_receiver = GroupCode::InterchangeId;
    let bytes = written(&spec, BALANCED_5010, &padded).unwrap();
    assert_eq!(written_element(&bytes, b"GS", 3), b"RECEIVER");
}

#[test]
fn a_date_is_checked_to_be_a_calendar_date() {
    assert_eq!(Envelope::date_of(2024, 2, 29), Ok(19_782));
    assert_eq!(Envelope::date_of(1970, 1, 1), Ok(0));
    assert_eq!(
        Envelope::date_of(2023, 2, 29).unwrap_err().to_string(),
        "date 2023-02-29 is not a calendar date"
    );
}

#[test]
fn a_time_is_checked_for_fractions_then_range() {
    assert_eq!(Envelope::time_of(23, 59, 59, 0), Ok(86_399));
    let text = |result: Result<i32, EnvelopeValueError>| result.unwrap_err().to_string();
    assert_eq!(
        text(Envelope::time_of(12, 30, 0, 500_000_000)),
        "time 12:30:00.5 has a fraction of a second; the envelope holds whole seconds"
    );
    assert_eq!(
        text(Envelope::time_of(24, 0, 0, 0)),
        "time 24:00:00 is out of range; it must be from 00:00:00 to 23:59:59"
    );
    assert_eq!(
        text(Envelope::time_of(12, 61, 0, 0)),
        "time 12:61:00 is out of range; it must be from 00:00:00 to 23:59:59"
    );
    assert!(Envelope::time_of(i64::MAX, 0, 0, 0).is_err());
    assert_eq!(
        text(Envelope::time_of(1, 0, 0, 1_000_000_000)),
        "time 01:00:00 is out of range; it must be from 00:00:00 to 23:59:59"
    );
}

#[test]
fn a_time_in_ticks_is_whole_seconds_in_range() {
    let text = |result: Result<i32, EnvelopeValueError>| result.unwrap_err().to_string();
    assert_eq!(
        Envelope::time_from_ticks(45_000_000_000, TimeUnit::Microsecond),
        Ok(45_000)
    );
    assert_eq!(
        text(Envelope::time_from_ticks(
            45_000_500_000,
            TimeUnit::Microsecond
        )),
        "time 12:30:00.5 has a fraction of a second; the envelope holds whole seconds"
    );
    assert_eq!(
        text(Envelope::time_from_ticks(86_400, TimeUnit::Second)),
        "time 24:00:00 is out of range; it must be from 00:00:00 to 23:59:59"
    );
    assert!(
        text(Envelope::time_from_ticks(-1_500, TimeUnit::Millisecond))
            .starts_with("time -1500 milliseconds after midnight")
    );
    assert_eq!(
        text(Envelope::time_from_ticks(-1, TimeUnit::Second)),
        "time -1 second after midnight is before midnight; it must be from 00:00:00 to 23:59:59"
    );
}
