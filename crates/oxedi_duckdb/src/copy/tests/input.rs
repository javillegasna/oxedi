use oxedi_core::ColumnType;

use super::super::error::CopyError;
use super::super::input::{BoundField, FieldType, InputColumn, InputField, Integer, bind};
use crate::builtins::{Builtins, TableSchema};

fn tables() -> Vec<TableSchema> {
    Builtins::load().default().tables.clone()
}

fn schema(table: &str) -> Vec<(String, ColumnType)> {
    tables()
        .into_iter()
        .find(|schema| schema.name == table)
        .map(|schema| schema.columns)
        .unwrap_or_default()
}

/// A field of the type a parse gives the column.
fn natural(name: &str, kind: ColumnType) -> InputField {
    let (kind, sql) = match kind {
        ColumnType::Binary => (FieldType::Varchar, "VARCHAR".to_owned()),
        ColumnType::Int64 { .. } => (FieldType::Integer(Integer::I64), "BIGINT".to_owned()),
        ColumnType::Decimal128 { scale, .. } => (
            FieldType::Decimal { width: 38, scale },
            format!("DECIMAL(38,{scale})"),
        ),
        ColumnType::Date32 => (FieldType::Date, "DATE".to_owned()),
        ColumnType::Time32 => (FieldType::Time, "TIME".to_owned()),
    };
    InputField {
        name: name.to_owned(),
        kind,
        sql,
    }
}

/// Every column of `table`, as `SELECT list(t) FROM table t` gives them.
fn whole(table: &str) -> InputColumn {
    InputColumn::Rows(
        schema(table)
            .iter()
            .map(|(name, kind)| natural(name, *kind))
            .collect(),
    )
}

fn named(fields: &[&str]) -> InputColumn {
    InputColumn::Rows(
        fields
            .iter()
            .map(|name| InputField {
                name: (*name).to_owned(),
                kind: FieldType::Null,
                sql: "NULL".to_owned(),
            })
            .collect(),
    )
}

fn message(columns: &[InputColumn]) -> String {
    match bind(&tables(), columns) {
        Ok(bound) => format!("bound: {bound:?}"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn every_table_is_found_by_its_fields() {
    let names = [
        "payments",
        "claims",
        "services",
        "adjustments",
        "provider_adjustments",
    ];
    let columns: Vec<InputColumn> = names.iter().map(|name| whole(name)).collect();
    let bound = bind(&tables(), &columns).ok();
    let found: Option<Vec<String>> =
        bound.map(|bound| bound.into_iter().map(|column| column.table).collect());
    assert_eq!(
        found,
        Some(names.iter().map(|name| (*name).to_owned()).collect())
    );
}

#[test]
fn a_subset_that_one_table_has_is_enough() {
    let bound = bind(&tables(), &[named(&["claim_id", "charge_amount"])]).ok();
    assert_eq!(
        bound.map(|bound| bound
            .into_iter()
            .map(|column| column.table)
            .collect::<Vec<_>>()),
        Some(vec!["claims".to_owned()])
    );
}

#[test]
fn fields_keep_their_order_and_input_type() {
    let column = InputColumn::Rows(vec![
        InputField {
            name: "payment_amount".to_owned(),
            kind: FieldType::Integer(Integer::I32),
            sql: "INTEGER".to_owned(),
        },
        natural("claim_id", ColumnType::Binary),
    ]);
    let bound = bind(&tables(), &[column]).ok();
    assert_eq!(
        bound
            .and_then(|bound| bound.into_iter().next())
            .map(|column| column.fields),
        Some(vec![
            BoundField {
                name: "payment_amount".to_owned(),
                kind: ColumnType::Decimal128 {
                    precision: 38,
                    scale: 2,
                },
                input: FieldType::Integer(Integer::I32),
            },
            BoundField {
                name: "claim_id".to_owned(),
                kind: ColumnType::Binary,
                input: FieldType::Varchar,
            },
        ])
    );
}

#[test]
fn a_column_that_is_not_a_list_of_structs() {
    assert_eq!(
        message(&[whole("claims"), InputColumn::Other("INTEGER[]".to_owned())]),
        "edi835: input column 2 is INTEGER[]; each input column must be a list of structs \
         holding one table's rows, such as (SELECT list(c) FROM claims c)"
    );
}

#[test]
fn fields_several_tables_have() {
    assert_eq!(
        message(&[named(&["row", "payment", "amount", "reason_code"])]),
        "edi835: input column 1 holds structs with the fields \"row\", \"payment\", \"amount\", \
         \"reason_code\", which the tables \"adjustments\", \"provider_adjustments\" all have; \
         add the fields that tell them apart, such as every column of the table"
    );
}

#[test]
fn a_field_the_best_table_lacks_reads_as_python() {
    assert_eq!(
        message(&[named(&["claim_id", "charge_amount", "nope"])]),
        format!(
            "edi835: table \"claims\" has no column \"nope\" in the spec; its columns are {}",
            schema("claims")
                .iter()
                .map(|(name, _)| format!("{name:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    );
}

#[test]
fn fields_no_table_has() {
    assert_eq!(
        message(&[named(&["nope", "other"])]),
        "edi835: input column 1 holds structs with the fields \"nope\", \"other\", which no \
         table of the spec has together; the spec's tables are \"adjustments\", \"claims\", \
         \"payments\", \"provider_adjustments\", \"services\""
    );
    assert_eq!(
        message(&[named(&["row", "nope"])]),
        "edi835: input column 1 holds structs with the fields \"row\", \"nope\", which no table \
         of the spec has together; the spec's tables are \"adjustments\", \"claims\", \"payments\", \
         \"provider_adjustments\", \"services\""
    );
}

#[test]
fn a_table_given_twice() {
    assert_eq!(
        message(&[whole("claims"), whole("services"), named(&["claim_id"])]),
        "edi835: input columns 1 and 3 both hold table \"claims\"; give each table once, with \
         every row in one list"
    );
}

#[test]
fn a_field_of_the_wrong_type() {
    let field = |name: &str, kind: FieldType, sql: &str| {
        InputColumn::Rows(vec![
            natural("claim_id", ColumnType::Binary),
            InputField {
                name: name.to_owned(),
                kind,
                sql: sql.to_owned(),
            },
        ])
    };
    assert_eq!(
        message(&[field("charge_amount", FieldType::Varchar, "VARCHAR")]),
        "edi835: table \"claims\" field \"charge_amount\" is VARCHAR; the spec's column is \
         decimal128(38, 2), which takes DECIMAL or an integer type"
    );
    assert_eq!(
        message(&[field("charge_amount", FieldType::Double, "DOUBLE")]),
        "edi835: table \"claims\" field \"charge_amount\" is DOUBLE, which is refused for the \
         spec's decimal128(38, 2) values: a float may not hold the amount exactly; cast it to \
         DECIMAL"
    );
    assert_eq!(
        message(&[field(
            "claim_id",
            FieldType::Integer(Integer::I32),
            "INTEGER"
        )]),
        "edi835: table \"claims\" field \"claim_id\" is INTEGER; the spec's column is binary, \
         which takes VARCHAR or BLOB"
    );
    assert_eq!(
        message(&[field("row", FieldType::Varchar, "VARCHAR")]),
        "edi835: table \"claims\" field \"row\" is VARCHAR; the spec's column is int64, which \
         takes an integer type or a FLOAT or DOUBLE holding whole numbers"
    );
    assert_eq!(
        message(&[field("statement_from", FieldType::Time, "TIME")]),
        "edi835: table \"claims\" field \"statement_from\" is TIME; the spec's column is \
         date32, which takes DATE, or TIMESTAMP at midnight"
    );
    assert_eq!(
        message(&[field(
            "statement_from",
            FieldType::Other,
            "TIMESTAMP WITH TIME ZONE"
        )]),
        "edi835: table \"claims\" field \"statement_from\" is TIMESTAMP WITH TIME ZONE; the \
         spec's column is date32, which takes DATE, or TIMESTAMP at midnight"
    );
}

#[test]
fn a_null_field_fits_any_column() {
    assert!(bind(&tables(), &[named(&["claim_id", "charge_amount"])]).is_ok());
    assert!(matches!(
        bind(&tables(), &[named(&["nope"])]),
        Err(CopyError::NoTable { .. })
    ));
}
