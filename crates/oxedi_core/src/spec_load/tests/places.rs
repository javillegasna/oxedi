//! The type and target compiled for each declared column.

use crate::spec::test_support::builtin;
use crate::spec::{ColumnPlace, Spec, Target, table_columns};
use crate::spec_load::Version;
use crate::value::ColumnType;

use super::compat;

/// The place of column `column` of table `table`, by name.
fn place<'s>(spec: &'s Spec, table: &str, column: &str) -> &'s ColumnPlace {
    let at = spec
        .tables()
        .iter()
        .position(|def| def.name == table)
        .unwrap_or_else(|| panic!("no table {table}"));
    let name = spec.tables()[at]
        .columns
        .iter()
        .position(|(name, _)| name == column)
        .unwrap_or_else(|| panic!("no column {table}.{column}"));
    &spec.places(at)[name]
}

#[test]
fn each_column_place_has_its_schema_type() {
    let compat = compat();
    let five = builtin(Version::V5010);
    let four = builtin(Version::V4010);
    for spec in [&five.spec, &four.spec, &compat.spec] {
        for (at, def) in spec.tables().iter().enumerate() {
            let kinds: Vec<ColumnType> = spec.places(at).iter().map(|p| p.kind).collect();
            let schema: Vec<ColumnType> = table_columns(spec, def)
                .into_iter()
                .skip(2 + def.ancestors.len())
                .map(|(_, kind)| kind)
                .collect();
            assert_eq!(kinds, schema, "table {}", def.name);
        }
    }
}

#[test]
fn a_column_place_names_what_of_the_segment_it_reads() {
    let five = builtin(Version::V5010);
    let compat = compat();
    let code = place(&five.spec, "services", "procedure_code");
    assert_eq!(
        code.target,
        Target::Element {
            element: 1,
            component: Some(2)
        }
    );
    let amount = place(&five.spec, "adjustments", "amount");
    assert_eq!(
        amount.target,
        Target::Group {
            offset: 1,
            component: None
        }
    );
    assert_eq!(
        amount.kind,
        ColumnType::Decimal128 {
            precision: 38,
            scale: 2
        }
    );
    let index = place(&compat.spec, "rows", "service_date_segment");
    assert_eq!(index.target, Target::Index);
    assert_eq!(index.kind, ColumnType::Int64 { scale: 0 });
    assert_eq!(
        place(&five.spec, "claims", "header_number").kind,
        ColumnType::Int64 { scale: 0 }
    );
}
