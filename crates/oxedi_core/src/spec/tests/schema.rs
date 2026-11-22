//! The schema of a spec's tables, known without a file.

use crate::spec::test_support::{builtin, patched, spec};
use crate::spec::{ColumnSchema, Spec, Version, require_column, spec_schema, table_columns};
use crate::value::ColumnType;

const INDEX: ColumnType = ColumnType::Int64 { scale: 0 };

fn column(name: &str, kind: ColumnType, parent: Option<&str>) -> ColumnSchema {
    ColumnSchema {
        name: name.to_string(),
        kind,
        parent: parent.map(str::to_string),
    }
}

#[test]
fn reference_columns_name_the_table_they_point_at() {
    let schema = spec_schema(&spec());
    let lines = schema.iter().find(|table| table.name == "lines").unwrap();
    assert_eq!(
        lines.columns,
        vec![
            column("row", INDEX, None),
            column("segment", INDEX, None),
            column("head", INDEX, Some("heads")),
            column("claim", INDEX, Some("claims")),
            column("code", ColumnType::Binary, None),
            column("date", ColumnType::Date32, None),
            column("time", ColumnType::Time32, None),
        ]
    );
    let heads = schema.iter().find(|table| table.name == "heads").unwrap();
    assert!(heads.columns.iter().all(|column| column.parent.is_none()));
}

#[test]
fn the_tables_come_in_spec_order_with_their_columns() {
    let spec = spec();
    let schema = spec_schema(&spec);
    let names: Vec<&str> = schema.iter().map(|table| table.name.as_str()).collect();
    assert_eq!(names, ["adjustments", "claims", "heads", "lines"]);
    for (table, def) in schema.iter().zip(spec.tables()) {
        let pairs: Vec<(String, ColumnType)> = table
            .columns
            .iter()
            .map(|column| (column.name.clone(), column.kind))
            .collect();
        assert_eq!(pairs, table_columns(&spec, def));
    }
}

#[test]
fn a_patched_column_takes_its_element_type() {
    let spec = patched(
        &builtin(Version::V5010),
        r#"{"tables":{"payments":{"columns":{"check_date":{"segment":"BPR","element":16}}}}}"#,
    )
    .unwrap();
    let schema = spec_schema(&spec);
    let payments = schema
        .iter()
        .find(|table| table.name == "payments")
        .unwrap();
    let check_date = payments
        .columns
        .iter()
        .find(|column| column.name == "check_date")
        .unwrap();
    assert_eq!(check_date, &column("check_date", ColumnType::Date32, None));
}

fn two_columns() -> Vec<(String, ColumnType)> {
    vec![
        ("row".to_owned(), INDEX),
        ("claim_id".to_owned(), ColumnType::Binary),
    ]
}

#[test]
fn require_column_returns_the_position_and_type() {
    assert_eq!(
        require_column("claims", &two_columns(), "claim_id"),
        Ok((1, ColumnType::Binary))
    );
}

#[test]
fn require_column_names_every_column_in_schema_order() {
    let error = require_column("claims", &two_columns(), "nope").unwrap_err();
    assert_eq!(error.table, "claims");
    assert_eq!(error.column, "nope");
    assert_eq!(error.columns, ["row", "claim_id"]);
}

#[test]
fn table_def_require_column_lists_the_declared_columns() {
    let spec = Spec::builtin_835();
    let table = spec.table(spec.default_table().unwrap()).unwrap();
    let declared: Vec<&str> = table
        .columns
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    let error = table.require_column("nope").unwrap_err();
    assert_eq!(error.table, table.name);
    assert_eq!(error.column, "nope");
    assert_eq!(error.columns, declared);
    assert!(table.require_column(declared[0]).is_ok());
}
