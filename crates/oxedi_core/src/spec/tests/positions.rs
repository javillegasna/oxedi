//! The positions a column reads, from its compiled place.

use crate::spec::{ColumnSource, Spec, TableDef, test_support};

fn column_of(table: &TableDef, name: &str) -> usize {
    table
        .columns
        .iter()
        .position(|(column, _)| column == name)
        .unwrap()
}

#[test]
fn positions_follow_the_column_place() {
    let spec = Spec::builtin_835();
    let adjustments = spec.table("adjustments").unwrap();
    let reason = spec.positions(adjustments, column_of(adjustments, "reason_code"));
    assert_eq!(
        reason,
        [2, 5, 8, 11, 14, 17].map(|element| (&b"CAS"[..], element, None))
    );
    let claims = spec.table("claims").unwrap();
    let charge = spec.positions(claims, column_of(claims, "charge_amount"));
    assert_eq!(charge, [(&b"CLP"[..], 3, None)]);
    let small = test_support::spec();
    let table = small
        .tables()
        .iter()
        .find(|table| table.column("first_line_at").is_some())
        .unwrap();
    let at = column_of(table, "first_line_at");
    assert!(matches!(
        table.columns[at].1,
        ColumnSource::SegmentIndex { .. }
    ));
    assert!(small.positions(table, at).is_empty());
}
