use oxedi_core::diagnostic::{LoopRef, Rule, SnipLevel};
use oxedi_core::{Cell, Diagnostic, Document};

use super::{COLUMNS, of_findings, of_unparsable, types};
use crate::error::ReadError;
use crate::types::SqlType;

#[test]
fn only_datum_follows_binary() {
    let Ok(text) = types(false) else {
        panic!("every column has a DuckDB type");
    };
    let Ok(binary) = types(true) else {
        panic!("every column has a DuckDB type");
    };
    let changed: Vec<usize> = text
        .iter()
        .zip(&binary)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(index, _)| index)
        .collect();
    assert_eq!(changed, vec![7]);
    assert_eq!(binary.get(7), Some(&SqlType::Blob));
    assert_eq!(text.first(), Some(&SqlType::Bigint));
}

#[test]
fn of_findings_writes_each_column_from_the_shape() {
    let path = vec![
        LoopRef {
            name: "1000A".into(),
            ordinal: 1,
        },
        LoopRef {
            name: "2100".into(),
            ordinal: 2,
        },
    ];
    let own = Rule::UnknownSegment {
        segment_id: b"XX".to_vec(),
    };
    let rule = own.to_string();
    let diagnostics = [
        Diagnostic::new(own, Some(7), None, None, path, b"XX".to_vec()),
        Diagnostic::new(
            Rule::External {
                origin: "pyx12".into(),
                code: Some("8".into()),
                message: "Mandatory data element missing".into(),
                level: SnipLevel::L2,
            },
            Some(3),
            Some(2),
            Some(1),
            Vec::new(),
            Vec::new(),
        ),
    ];
    let Ok(table) = of_findings(&diagnostics) else {
        panic!("every diagnostic is of a file");
    };
    assert_eq!(table.len(), 2);
    let row = |row: usize| -> Vec<Option<Cell<'_>>> {
        COLUMNS
            .iter()
            .map(|(name, ..)| table.column(name).and_then(|column| column.get(row)))
            .collect()
    };
    let text = |text: &'static [u8]| Some(Cell::Binary(text));
    let int = |value: i64| Some(Cell::Int64(value));
    let null = Some(Cell::Null);
    assert_eq!(
        row(0),
        [
            int(1),
            text(b"UnknownSegment"),
            Some(Cell::Binary(rule.as_bytes())),
            int(7),
            null,
            null,
            text(b"1000A#1/2100#2"),
            text(b"XX"),
            text(b"oxedi"),
            null,
        ]
    );
    assert_eq!(
        row(1),
        [
            int(2),
            text(b"External"),
            Some(Cell::Binary(diagnostics[1].rule.to_string().as_bytes())),
            int(3),
            int(2),
            int(1),
            text(b""),
            text(b""),
            text(b"pyx12"),
            text(b"8"),
        ]
    );
}

#[test]
fn an_unparsable_file_is_one_row() {
    let Err(source) = Document::parse(&b"hello"[..]) else {
        panic!("the input has no ISA");
    };
    let error = ReadError::Parse {
        file: "a.835".to_owned(),
        source,
    };
    let Ok(table) = of_unparsable(&error, b"hello", false) else {
        panic!("the row fits the table");
    };
    assert_eq!(table.len(), 1);
    let cell = |name: &str| table.column(name).and_then(|column| column.get(0));
    assert_eq!(cell("level"), Some(Cell::Int64(1)));
    assert_eq!(cell("kind"), Some(Cell::Binary(b"NotAnInterchange")));
    assert_eq!(
        cell("rule"),
        Some(Cell::Binary(error.to_string().as_bytes()))
    );
    assert_eq!(cell("segment"), Some(Cell::Null));
    assert_eq!(cell("path"), Some(Cell::Binary(b"")));
    assert_eq!(cell("datum"), Some(Cell::Binary(b"hello")));
    assert_eq!(cell("origin"), Some(Cell::Binary(b"read_835")));
    assert_eq!(cell("code"), Some(Cell::Null));
}

fn datum_of_binary_file(binary: bool) -> Vec<u8> {
    let bytes = b"\x1f\x8b\x08\x00\x00\x00\x00\x00rest".to_vec();
    let Err(source) = Document::parse(bytes.as_slice()) else {
        panic!("the input has no ISA");
    };
    let found = source.datum(&bytes).to_vec();
    let error = ReadError::Parse {
        file: "a.gz".to_owned(),
        source,
    };
    let Ok(table) = of_unparsable(&error, &found, binary) else {
        panic!("the row fits the table");
    };
    match table.column("datum").and_then(|column| column.get(0)) {
        Some(Cell::Binary(bytes)) => bytes.to_vec(),
        other => panic!("the datum is a binary cell, not {other:?}"),
    }
}

#[test]
fn the_datum_of_a_binary_file_is_escaped_text_in_varchar_mode() {
    let escaped = datum_of_binary_file(false);
    assert_eq!(escaped, br"\x1f\x8b\x08\x00\x00\x00\x00\x00");
    assert!(std::str::from_utf8(&escaped).is_ok());
}

#[test]
fn the_datum_of_a_binary_file_is_raw_in_binary_mode() {
    assert_eq!(
        datum_of_binary_file(true),
        b"\x1f\x8b\x08\x00\x00\x00\x00\x00"
    );
}
