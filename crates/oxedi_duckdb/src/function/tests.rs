use super::{c_message, guarded, panic_message};
use crate::error::ReadError;

#[test]
fn a_panic_becomes_an_internal_error() {
    let result: Result<(), ReadError> = guarded(|| panic!("boom {}", 1));
    assert_eq!(
        result.map_err(|error| error.to_string()),
        Err("read_835: internal error: boom 1".to_owned())
    );
}

#[test]
fn an_error_passes_through() {
    let result: Result<(), ReadError> = guarded(|| Err(ReadError::EmptyPathList));
    assert!(matches!(result, Err(ReadError::EmptyPathList)));
}

#[test]
fn panic_payloads_without_text() {
    assert_eq!(panic_message(&42u8), "a panic without a message");
    assert_eq!(panic_message(&"static"), "static");
}

#[test]
fn nul_bytes_are_written_out() {
    let error = ReadError::Internal {
        message: "a\0b".to_owned(),
    };
    assert_eq!(
        c_message(&error).to_str().ok(),
        Some("read_835: internal error: a\\0b")
    );
}
