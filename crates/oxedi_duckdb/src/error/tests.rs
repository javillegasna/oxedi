use std::error::Error;

use edi835_core::{ColumnType, Document};

use super::ReadError;

fn text(error: ReadError) -> String {
    error.to_string()
}

#[test]
fn path_type() {
    assert_eq!(
        text(ReadError::PathType {
            found: "Integer".to_owned()
        }),
        "read_835: the path must be a VARCHAR or a list of VARCHAR; found Integer"
    );
}

#[test]
fn null_path() {
    assert_eq!(
        text(ReadError::NullPath { position: None }),
        "read_835: the path must not be NULL"
    );
}

#[test]
fn null_path_in_list() {
    assert_eq!(
        text(ReadError::NullPath { position: Some(2) }),
        "read_835: every path of the list must be a string; path 2 is NULL"
    );
}

#[test]
fn empty_path_list() {
    assert_eq!(
        text(ReadError::EmptyPathList),
        "read_835: the list of paths must not be empty"
    );
}

#[test]
fn unknown_table() {
    assert_eq!(
        text(ReadError::UnknownTable {
            table: "nope".to_owned(),
            known: vec!["claims".to_owned(), "services".to_owned()],
        }),
        "read_835: unknown table \"nope\"; table must be one of \"claims\", \"services\""
    );
}

#[test]
fn unknown_version() {
    assert_eq!(
        text(ReadError::UnknownVersion {
            version: "3070".to_owned(),
            known: vec!["5010", "4010"],
        }),
        "read_835: unknown version \"3070\"; version must be one of \"5010\", \"4010\""
    );
}

#[test]
fn open() {
    assert_eq!(
        text(ReadError::Open {
            file: "a.835".to_owned(),
            message: "No such file or directory".to_owned(),
        }),
        "read_835: \"a.835\" could not be opened: No such file or directory"
    );
}

#[test]
fn read() {
    assert_eq!(
        text(ReadError::Read {
            file: "a.835".to_owned(),
            message: "connection reset".to_owned(),
        }),
        "read_835: \"a.835\" could not be read: connection reset"
    );
}

#[test]
fn parse_names_the_file_and_chains_the_core_error() {
    let Err(source) = Document::parse(b"not an interchange".to_vec()) else {
        panic!("the input has no ISA");
    };
    let core = source.to_string();
    let error = ReadError::Parse {
        file: "a.835".to_owned(),
        source,
    };
    assert_eq!(
        error.to_string(),
        format!("read_835: \"a.835\" is not an X12 interchange: {core}")
    );
    assert_eq!(error.source().map(ToString::to_string), Some(core));
}

#[test]
fn invalid_utf8() {
    assert_eq!(
        text(ReadError::InvalidUtf8 {
            file: "a.835".to_owned(),
            table: "claims".to_owned(),
            column: "patient_last_name".to_owned(),
            row: 3,
            bytes: b"M\xfcLLER".to_vec(),
        }),
        "read_835: \"a.835\", table \"claims\", column \"patient_last_name\", row 3: \
         a VARCHAR must be valid UTF-8; found b\"M\\xfcLLER\"; \
         pass binary := true to read text columns as BLOB"
    );
}

#[test]
fn schema_mismatch() {
    assert_eq!(
        text(ReadError::SchemaMismatch {
            file: "a.835".to_owned(),
            table: "claims".to_owned(),
            version: "4010",
            bound: "5010",
        }),
        "read_835: \"a.835\" declares version 4010, whose table \"claims\" has other columns \
         than version 5010, which the query was bound with; pass version := '4010' to read \
         such files on their own"
    );
}

#[test]
fn unsupported_type() {
    assert_eq!(
        text(ReadError::UnsupportedType {
            table: "claims".to_owned(),
            column: "charge_amount".to_owned(),
            kind: ColumnType::Decimal128 {
                precision: 9,
                scale: 2
            },
        }),
        "read_835: table \"claims\", column \"charge_amount\": the core type \
         decimal128(9, 2) has no DuckDB type"
    );
}

#[test]
fn internal() {
    assert_eq!(
        text(ReadError::Internal {
            message: "boom".to_owned()
        }),
        "read_835: internal error: boom"
    );
}

#[test]
fn only_parse_has_a_source() {
    assert!(ReadError::EmptyPathList.source().is_none());
}
