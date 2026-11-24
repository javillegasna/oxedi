use super::*;
use crate::spec_load::SpecError;

const EXAMPLE: &str = r#"{
  "kit": {"format": 1, "package": "oxedi/views/005010/835/X221A1"},
  "cases": {
    "balanced_5010_sample": {
      "input": "../../tests/fixtures/balanced_5010_sample.txt",
      "expect": {
        "tables": "../../tests/golden/project/balanced_5010_sample.txt.tables.txt",
        "diagnostics": "../../tests/golden/project/balanced_5010_sample.txt.diagnostics.txt"
      }
    }
  }
}"#;

fn kit_with(cases: &str) -> String {
    format!(r#"{{"kit":{{"format":1,"package":"oxedi/views/005010/835/X221A1"}},"cases":{cases}}}"#)
}

fn error_of(cases: &str) -> SpecError {
    Kit::from_json(&kit_with(cases)).unwrap_err()
}

#[test]
fn the_example_manifest_reads() {
    let kit = Kit::from_json(EXAMPLE).unwrap();
    assert_eq!(kit.header().format.get(), 1);
    assert_eq!(
        kit.header().package.as_str(),
        "oxedi/views/005010/835/X221A1"
    );
    assert_eq!(kit.cases().len(), 1);
    let case = &kit.cases()["balanced_5010_sample"];
    assert_eq!(
        case.expect.keys().copied().collect::<Vec<_>>(),
        [KitOutput::Tables, KitOutput::Diagnostics]
    );
    assert_eq!(
        kit.files(),
        [
            "../../tests/fixtures/balanced_5010_sample.txt",
            "../../tests/golden/project/balanced_5010_sample.txt.diagnostics.txt",
            "../../tests/golden/project/balanced_5010_sample.txt.tables.txt",
        ]
    );
}

#[test]
fn a_newer_kit_format_is_refused_first() {
    let err = Kit::from_json(r#"{"kit":{"format":2,"package":"x"},"extra":1}"#).unwrap_err();
    assert!(matches!(
        err,
        SpecError::BadKit {
            reason: KitError::FormatTooNew {
                found: 2,
                reader: 1
            }
        }
    ));
    assert_eq!(
        err.to_string(),
        "the kit is invalid: kit format 2 is newer than 1, the newest format this reader takes; read the kit with a newer oxedi, or write it in format 1"
    );
}

#[test]
fn an_unknown_key_is_refused_with_its_path() {
    let text = error_of(r#"{"a":{"input":"x","expect":{"tables":"y"},"extra":1}}"#).to_string();
    assert!(text.contains("cases.a"), "{text}");
    assert!(text.contains("extra"), "{text}");
}

#[test]
fn no_cases_display() {
    assert_eq!(
        error_of("{}").to_string(),
        "the kit is invalid: the kit has no case; add one under \"cases\""
    );
}

#[test]
fn no_outputs_display() {
    assert_eq!(
        error_of(r#"{"a":{"input":"x","expect":{}}}"#).to_string(),
        "the kit is invalid: case \"a\" expects no output; name a file under \"expect\" for \"tables\" or \"diagnostics\""
    );
}

#[test]
fn an_unknown_output_is_refused() {
    let text = error_of(r#"{"a":{"input":"x","expect":{"events":"y"}}}"#).to_string();
    assert!(
        text.contains("\"output\" must be \"tables\" or \"diagnostics\"; found \"events\""),
        "{text}"
    );
}

#[test]
fn bad_paths_are_refused() {
    for path in ["/abs", "a\\\\b", "a//b", "", "a/"] {
        let text = error_of(&format!(
            r#"{{"a":{{"input":"{path}","expect":{{"tables":"y"}}}}}}"#
        ))
        .to_string();
        assert!(
            text.contains("a path must be relative and written with \"/\""),
            "{path:?}: {text}"
        );
    }
}

#[test]
fn bad_kit_has_no_source() {
    let err = error_of("{}");
    assert!(std::error::Error::source(&err).is_none());
}
