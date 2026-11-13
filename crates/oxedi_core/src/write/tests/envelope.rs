use super::super::envelope::interchange_version;
use super::super::{Envelope, Finding, WriteError, write, write_with_findings};
use crate::delimiters::Delimiters;
use crate::diagnostic::Rule;
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
    let spec = Spec::builtin_835()
        .merge_patch(r#"{"segments":{"ISA":{"elements":{"11":{"codes":["U"]}}}}}"#)
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
    let spec = Spec::builtin_835_4010()
        .merge_patch(r#"{"segments":{"ISA":{"elements":{"11":{"codes":null,"required":false}}}}}"#)
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
        Err(WriteError::NoRepetition)
    ));
}

#[test]
fn a_spec_with_no_value_for_isa12_writes_no_separator() {
    let spec = Spec::builtin_835()
        .merge_patch(
            r#"{"segments":{"ISA":{"elements":{"11":{"required":false},"12":{"codes":null,"required":false}}}}}"#,
        )
        .unwrap();
    let envelope = envelope_with(Delimiters::new(b'*', b':', b'~').with_repetition(b'^'));
    let bytes = written(&spec, BALANCED_5010, &envelope).unwrap();
    assert_eq!(Delimiters::from_isa(&bytes).unwrap().repetition, None);
}

#[test]
fn a_declared_version_that_is_not_isa12_does_not_decide_the_separator() {
    let spec = Spec::builtin_835_4010()
        .merge_patch(
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

#[test]
fn the_version_the_rule_reads_is_the_isa12_the_writer_writes() {
    let patched = Spec::builtin_835()
        .merge_patch(r#"{"segments":{"ISA":{"elements":{"12":{"codes":["00402"]}}}}}"#)
        .unwrap();
    let declared = Spec::builtin_835()
        .merge_patch(
            r#"{"version":{"segment":"ISA","element":12,"values":["00401"]},
                "segments":{"ISA":{"elements":{"11":{"required":false},"12":{"codes":["00401"]}}}}}"#,
        )
        .unwrap();
    let envelope = envelope_with(Delimiters::new(b'*', b':', b'~').with_repetition(b'^'));
    for (spec, input) in [
        (Spec::builtin_835(), BALANCED_5010),
        (Spec::builtin_835_4010(), BALANCED_4010),
        (patched, BALANCED_5010),
        (declared, BALANCED_5010),
    ] {
        let bytes = written(&spec, input, &envelope).unwrap();
        let isa = bytes.split(|&byte| byte == b'~').next().unwrap();
        let isa12 = isa.split(|&byte| byte == b'*').nth(12).unwrap();
        assert_eq!(interchange_version(&spec, b"ISA"), isa12);
    }
}
