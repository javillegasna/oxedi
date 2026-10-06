use oxedi_core::ColumnType;

use super::super::error::CopyError;
use super::super::input::{
    BoundField, FieldType, InputColumn, InputField, InputTable, Integer, bind,
};
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

fn quoted_columns(table: &str) -> String {
    schema(table)
        .iter()
        .map(|(name, _)| format!("{name:?}"))
        .collect::<Vec<_>>()
        .join(", ")
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
fn whole(table: &str) -> (String, InputTable) {
    (
        table.to_owned(),
        InputTable::Rows(
            schema(table)
                .iter()
                .map(|(name, kind)| natural(name, *kind))
                .collect(),
        ),
    )
}

fn input(tables: Vec<(String, InputTable)>) -> Vec<InputColumn> {
    vec![InputColumn::Tables(tables)]
}

fn message(columns: &[InputColumn]) -> String {
    match bind(&tables(), columns) {
        Ok(bound) => format!("bound: {bound:?}"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn every_table_binds_by_name_in_the_struct_order() {
    let names = [
        "services",
        "payments",
        "claims",
        "provider_adjustments",
        "adjustments",
    ];
    let bound = bind(
        &tables(),
        &input(names.iter().map(|name| whole(name)).collect()),
    )
    .ok();
    assert_eq!(
        bound.map(|bound| bound
            .into_iter()
            .map(|column| column.table)
            .collect::<Vec<_>>()),
        Some(names.iter().map(|name| (*name).to_owned()).collect())
    );
}

#[test]
fn fields_keep_their_order_and_input_type() {
    let claims = InputTable::Rows(vec![
        InputField {
            name: "payment_amount".to_owned(),
            kind: FieldType::Integer(Integer::I32),
            sql: "INTEGER".to_owned(),
        },
        natural("claim_id", ColumnType::Binary),
    ]);
    let bound = bind(&tables(), &input(vec![("claims".to_owned(), claims)])).ok();
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
fn an_empty_struct_binds_no_table() {
    assert_eq!(bind(&tables(), &input(Vec::new())).ok(), Some(Vec::new()));
}

#[test]
fn the_query_must_return_one_column() {
    let one = InputColumn::Tables(vec![whole("claims")]);
    assert_eq!(
        message(&[one.clone(), one]),
        "edi835: the query returns 2 columns; it must return one, a STRUCT with one field per \
         table, such as SELECT {'claims': (SELECT list(c) FROM claims c)}"
    );
    assert!(matches!(
        bind(&tables(), &[]),
        Err(CopyError::ColumnCount { found: 0 })
    ));
}

#[test]
fn the_column_must_be_a_struct() {
    assert_eq!(
        message(&[InputColumn::Other("STRUCT(\"row\" BIGINT)[]".to_owned())]),
        "edi835: the query's column is STRUCT(\"row\" BIGINT)[]; it must be a STRUCT with one \
         field per table, such as SELECT {'claims': (SELECT list(c) FROM claims c)}"
    );
}

#[test]
fn an_unknown_table_reads_as_python() {
    assert_eq!(
        message(&input(vec![
            whole("claims"),
            ("nope".to_owned(), InputTable::Rows(Vec::new()))
        ])),
        "edi835: table \"nope\" is not a table of the spec, whose tables are \"adjustments\", \
         \"claims\", \"payments\", \"provider_adjustments\", \"services\""
    );
}

#[test]
fn a_table_must_be_a_list_of_structs() {
    assert_eq!(
        message(&input(vec![(
            "claims".to_owned(),
            InputTable::Other("VARCHAR[]".to_owned())
        )])),
        "edi835: table \"claims\" is VARCHAR[]; it must be a list of structs holding the table's \
         rows, such as (SELECT list(c) FROM claims c)"
    );
}

#[test]
fn an_unknown_column_reads_as_python() {
    let claims = InputTable::Rows(vec![
        natural("claim_id", ColumnType::Binary),
        natural("nope", ColumnType::Binary),
    ]);
    assert_eq!(
        message(&input(vec![("claims".to_owned(), claims)])),
        format!(
            "edi835: table \"claims\" has no column \"nope\" in the spec; its columns are {}",
            quoted_columns("claims")
        )
    );
}

#[test]
fn a_field_of_the_wrong_type() {
    let field = |name: &str, kind: FieldType, sql: &str| {
        input(vec![(
            "claims".to_owned(),
            InputTable::Rows(vec![InputField {
                name: name.to_owned(),
                kind,
                sql: sql.to_owned(),
            }]),
        )])
    };
    assert_eq!(
        message(&field("charge_amount", FieldType::Varchar, "VARCHAR")),
        "edi835: table \"claims\" field \"charge_amount\" is VARCHAR; the spec's column is \
         decimal128(38, 2), which takes DECIMAL or an integer type"
    );
    assert_eq!(
        message(&field("charge_amount", FieldType::Double, "DOUBLE")),
        "edi835: table \"claims\" field \"charge_amount\" is DOUBLE, which is refused for the \
         spec's decimal128(38, 2) values: a float may not hold the amount exactly; cast it to \
         DECIMAL"
    );
    assert_eq!(
        message(&field(
            "claim_id",
            FieldType::Integer(Integer::I32),
            "INTEGER"
        )),
        "edi835: table \"claims\" field \"claim_id\" is INTEGER; the spec's column is binary, \
         which takes VARCHAR or BLOB"
    );
    assert_eq!(
        message(&field("row", FieldType::Varchar, "VARCHAR")),
        "edi835: table \"claims\" field \"row\" is VARCHAR; the spec's column is int64, which \
         takes an integer type or a FLOAT or DOUBLE holding whole numbers"
    );
    assert_eq!(
        message(&field("statement_from", FieldType::Time, "TIME")),
        "edi835: table \"claims\" field \"statement_from\" is TIME; the spec's column is \
         date32, which takes DATE, or TIMESTAMP at midnight"
    );
    assert_eq!(
        message(&field(
            "statement_from",
            FieldType::Other,
            "TIMESTAMP WITH TIME ZONE"
        )),
        "edi835: table \"claims\" field \"statement_from\" is TIMESTAMP WITH TIME ZONE; the \
         spec's column is date32, which takes DATE, or TIMESTAMP at midnight"
    );
    assert!(bind(&tables(), &field("claim_id", FieldType::Null, "NULL")).is_ok());
}
