use crate::api::BatchBy;
use crate::spec::BatchLoopError;
use crate::spec::test_support::{builtin, nested};

fn unknown() -> BatchLoopError {
    BatchLoopError::Unknown {
        by: "x".into(),
        loops: vec!["a".into(), "b".into()],
    }
}

#[test]
fn the_innermost_envelope_is_the_transaction() {
    for builtin in crate::spec_load::Builtin::all() {
        let spec = &builtin.spec;
        assert_eq!(
            BatchBy::default().resolve(spec),
            Ok(spec.loop_id("transaction").unwrap())
        );
    }
}

#[test]
fn a_named_loop_resolves_to_its_id() {
    let spec = builtin(crate::spec_load::Version::V5010).spec;
    let by = BatchBy::Loop("2000".into());
    assert_eq!(by.resolve(&spec), Ok(spec.loop_id("2000").unwrap()));
}

#[test]
fn a_spec_without_envelope_loops_has_no_innermost() {
    assert_eq!(
        BatchBy::InnermostEnvelope.resolve(&nested()),
        Err(BatchLoopError::NoEnvelope)
    );
    let by = BatchBy::Loop("nope".into()).resolve(&nested());
    assert_eq!(
        by.unwrap_err().to_string(),
        r#"by "nope": the spec has no such loop; its loops are A, B, C"#
    );
}

#[test]
fn each_variant_writes_its_text() {
    assert_eq!(
        unknown().to_string(),
        r#"by "x": the spec has no such loop; its loops are a, b"#
    );
    assert_eq!(
        BatchLoopError::NoEnvelope.to_string(),
        "the spec has no envelope loop (a loop whose end segment checks it) to cut batches at; \
         name a loop"
    );
}
