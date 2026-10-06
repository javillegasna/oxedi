use std::error::Error;

use oxedi_core::ColumnType;
use oxedi_core::column::{CellError, RowError};
use oxedi_core::write::WriteError;

use super::super::error::CopyError;
use super::super::options::{DELIMITER_FIELDS, OPTIONS, REQUIRED};

const DECIMAL: ColumnType = ColumnType::Decimal128 {
    precision: 38,
    scale: 2,
};

fn text(error: CopyError) -> String {
    error.to_string()
}

#[test]
fn unknown_option() {
    assert_eq!(
        text(CopyError::UnknownOption {
            name: "header".to_owned(),
            known: OPTIONS,
        }),
        "edi835: unknown option \"header\"; the options are \"sender_id\", \"receiver_id\", \
         \"date\", \"time\", \"sender_qualifier\", \"receiver_qualifier\", \"usage_indicator\", \
         \"control_number\", \"application_sender\", \"application_receiver\", \"delimiters\", \
         \"line_break\", \"version\""
    );
}

#[test]
fn file_option() {
    assert_eq!(
        text(CopyError::FileOption {
            name: "compression".to_owned(),
            reason: "the format writes one uncompressed file; compress it afterwards",
        }),
        "edi835: the option \"compression\" does not apply: the format writes one uncompressed \
         file; compress it afterwards"
    );
}

#[test]
fn missing_option() {
    assert_eq!(
        text(CopyError::MissingOption {
            name: "date",
            required: REQUIRED,
        }),
        "edi835: the option \"date\" is required; every one of \"sender_id\", \"receiver_id\", \
         \"date\", \"time\" must be given"
    );
}

#[test]
fn no_value() {
    assert_eq!(
        text(CopyError::NoValue { name: "sender_id" }),
        "edi835: the option \"sender_id\" needs a value"
    );
}

#[test]
fn option_value() {
    assert_eq!(
        text(CopyError::OptionValue {
            name: "control_number",
            value: "INTEGER -1".to_owned(),
            expected: "a whole number from 0 to 18446744073709551615",
        }),
        "edi835: the option \"control_number\" is INTEGER -1; it must be a whole number from 0 \
         to 18446744073709551615"
    );
}

#[test]
fn fractional_time() {
    assert_eq!(
        text(CopyError::FractionalTime {
            value: "TIME 10:30:00.5".to_owned(),
        }),
        "edi835: the option \"time\" is TIME 10:30:00.5, which has a fraction of a second; the \
         envelope holds whole seconds"
    );
}

#[test]
fn unknown_version() {
    assert_eq!(
        text(CopyError::UnknownVersion {
            version: "3070".to_owned(),
            known: vec!["5010", "4010"],
        }),
        "edi835: unknown version \"3070\"; version must be one of \"5010\", \"4010\""
    );
}

#[test]
fn unknown_delimiter() {
    assert_eq!(
        text(CopyError::UnknownDelimiter {
            name: "sub".to_owned(),
            known: DELIMITER_FIELDS,
        }),
        "edi835: delimiters has no field \"sub\"; its fields are \"element\", \"component\", \
         \"segment\", \"repetition\", \"release\""
    );
}

#[test]
fn delimiter_length() {
    assert_eq!(
        text(CopyError::DelimiterLength {
            name: "element".to_owned(),
            bytes: b"**".to_vec(),
        }),
        "edi835: delimiter element must be exactly one byte, got 2 bytes: b'**'"
    );
}

#[test]
fn not_rows() {
    assert_eq!(
        text(CopyError::NotRows {
            column: 2,
            found: "INTEGER".to_owned(),
        }),
        "edi835: input column 2 is INTEGER; each input column must be a list of structs holding \
         one table's rows, such as (SELECT list(c) FROM claims c)"
    );
}

#[test]
fn no_table() {
    assert_eq!(
        text(CopyError::NoTable {
            column: 1,
            fields: vec!["a".to_owned(), "b".to_owned()],
            tables: vec!["claims".to_owned(), "payments".to_owned()],
        }),
        "edi835: input column 1 holds structs with the fields \"a\", \"b\", which no table of \
         the spec has together; the spec's tables are \"claims\", \"payments\""
    );
}

#[test]
fn ambiguous_table() {
    assert_eq!(
        text(CopyError::AmbiguousTable {
            column: 3,
            fields: vec!["row".to_owned(), "amount".to_owned()],
            tables: vec!["adjustments".to_owned(), "provider_adjustments".to_owned()],
        }),
        "edi835: input column 3 holds structs with the fields \"row\", \"amount\", which the \
         tables \"adjustments\", \"provider_adjustments\" all have; add the fields that tell \
         them apart, such as every column of the table"
    );
}

#[test]
fn duplicate_table() {
    assert_eq!(
        text(CopyError::DuplicateTable {
            table: "claims".to_owned(),
            first: 1,
            second: 3,
        }),
        "edi835: input columns 1 and 3 both hold table \"claims\"; give each table once, with \
         every row in one list"
    );
}

#[test]
fn field_type() {
    assert_eq!(
        text(CopyError::FieldType {
            table: "claims".to_owned(),
            field: "charge_amount".to_owned(),
            found: "VARCHAR".to_owned(),
            expected: DECIMAL,
        }),
        "edi835: table \"claims\" field \"charge_amount\" is VARCHAR; the spec's column is \
         decimal128(38, 2), which takes DECIMAL or an integer type"
    );
}

#[test]
fn float_for_decimal() {
    assert_eq!(
        text(CopyError::FloatForDecimal {
            table: "claims".to_owned(),
            field: "charge_amount".to_owned(),
            found: "DOUBLE".to_owned(),
            expected: DECIMAL,
        }),
        "edi835: table \"claims\" field \"charge_amount\" is DOUBLE, which is refused for the \
         spec's decimal128(38, 2) values: a float may not hold the amount exactly; cast it to \
         DECIMAL"
    );
}

#[test]
fn value() {
    assert_eq!(
        text(CopyError::Value {
            table: "claims".to_owned(),
            column: "charge_amount".to_owned(),
            row: 4,
            reason: "1234 at scale 3 has more decimals than the column's scale 2".to_owned(),
        }),
        "edi835: table \"claims\" column \"charge_amount\" row 4: 1234 at scale 3 has more \
         decimals than the column's scale 2"
    );
}

#[test]
fn row_chains_its_source() {
    let error = CopyError::Row(RowError::Cell {
        table: "claims".to_owned(),
        column: "claim_id".to_owned(),
        source: CellError::BinaryOverflow {
            bytes: 2_147_483_648,
        },
    });
    assert_eq!(
        error.to_string(),
        "edi835: table \"claims\" column \"claim_id\": a binary column holds at most 2147483647 \
         bytes; this value would bring it to 2147483648"
    );
    assert!(error.source().is_some());
}

#[test]
fn write_chains_its_source() {
    let error = CopyError::Write(WriteError::UnknownColumn {
        table: "claims".to_owned(),
        column: "nope".to_owned(),
        columns: vec!["row".to_owned(), "segment".to_owned()],
    });
    assert_eq!(
        error.to_string(),
        "edi835: table \"claims\" has no column \"nope\" in the spec; its columns are \"row\", \
         \"segment\""
    );
    assert!(error.source().is_some());
}

#[test]
fn second_file() {
    assert_eq!(
        text(CopyError::SecondFile {
            path: "out/p=2/data_0.".to_owned(),
        }),
        "edi835: the COPY asks for a second file, \"out/p=2/data_0.\", but the format writes all \
         the rows as one interchange in one file; PARTITION_BY and PER_THREAD_OUTPUT do not apply"
    );
}

#[test]
fn output() {
    let error = CopyError::Output {
        path: "s3://bucket/out.835".to_owned(),
        step: "written",
        message: "access denied".to_owned(),
    };
    assert_eq!(
        error.to_string(),
        "edi835: \"s3://bucket/out.835\" could not be written: access denied"
    );
    assert!(error.source().is_none());
}

#[test]
fn internal() {
    assert_eq!(
        text(CopyError::Internal {
            message: "boom".to_owned(),
        }),
        "edi835: internal error: boom"
    );
}
