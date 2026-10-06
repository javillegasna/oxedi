use edi835_core::{Cell, Document, DocumentError, IsaError, SizeError};

use super::{datum, of_unparsable, types};
use crate::error::ReadError;
use crate::schema::SqlType;

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
fn the_datum_of_a_file_without_isa_is_what_was_found() {
    let bytes = b"hello, this is not an interchange".to_vec();
    let Err(error) = Document::parse(bytes.as_slice()) else {
        panic!("the input has no ISA");
    };
    assert_eq!(datum(&error, &bytes), b"hello, t");
}

#[test]
fn the_datum_of_a_truncated_isa_skips_the_leading_trivia() {
    let bytes = b"\xEF\xBB\xBF  ISA*00*".to_vec();
    let error = DocumentError::Isa(IsaError::Truncated {
        len: bytes.len(),
        separators_found: 2,
        byte_order_mark: true,
        whitespace: 2,
    });
    assert_eq!(datum(&error, &bytes), b"ISA*00*");
}

#[test]
fn a_file_too_long_has_no_datum() {
    let error = DocumentError::Size(SizeError { len: usize::MAX });
    assert_eq!(datum(&error, b"ISA"), b"");
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
    let Ok(table) = of_unparsable(&error, b"hello") else {
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
