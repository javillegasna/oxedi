use super::super::{ColumnType, ElementType};

#[test]
fn column_type_of_each_element_type() {
    assert_eq!(ColumnType::of(None), ColumnType::Binary);
    assert_eq!(ColumnType::of(Some(ElementType::An)), ColumnType::Binary);
    assert_eq!(ColumnType::of(Some(ElementType::Id)), ColumnType::Binary);
    assert_eq!(
        ColumnType::of(Some(ElementType::N(2))),
        ColumnType::Int64 { scale: 2 }
    );
    assert_eq!(
        ColumnType::of(Some(ElementType::R { scale: 4 })),
        ColumnType::Decimal128 {
            precision: 38,
            scale: 4
        }
    );
    assert_eq!(ColumnType::of(Some(ElementType::Dt)), ColumnType::Date32);
    assert_eq!(ColumnType::of(Some(ElementType::Tm)), ColumnType::Time32);
}

#[test]
fn column_types_display_their_arrow_names() {
    assert_eq!(ColumnType::Binary.to_string(), "binary");
    assert_eq!(ColumnType::Int64 { scale: 0 }.to_string(), "int64");
    assert_eq!(
        ColumnType::Int64 { scale: 2 }.to_string(),
        "int64 (scale 2)"
    );
    assert_eq!(
        ColumnType::Decimal128 {
            precision: 38,
            scale: 2
        }
        .to_string(),
        "decimal128(38, 2)"
    );
    assert_eq!(ColumnType::Date32.to_string(), "date32");
    assert_eq!(ColumnType::Time32.to_string(), "time32 (seconds)");
}

#[test]
fn decimal_scale_of_each_column_type() {
    assert_eq!(
        ColumnType::Decimal128 {
            precision: 38,
            scale: 2
        }
        .decimal_scale(),
        Some(2)
    );
    assert_eq!(ColumnType::Int64 { scale: 2 }.decimal_scale(), None);
    assert_eq!(ColumnType::Binary.decimal_scale(), None);
    assert_eq!(ColumnType::Date32.decimal_scale(), None);
    assert_eq!(ColumnType::Time32.decimal_scale(), None);
}

#[test]
fn only_a_decimal_column_refuses_a_float() {
    let decimal = ColumnType::Decimal128 {
        precision: 18,
        scale: 2,
    };
    let error = decimal.takes_float().unwrap_err();
    assert_eq!(
        error.to_string(),
        "a float may not hold the spec's decimal128(18, 2) values exactly; give decimal values"
    );
    assert!(std::error::Error::source(&error).is_none());
    for column in [
        ColumnType::Int64 { scale: 0 },
        ColumnType::Binary,
        ColumnType::Date32,
        ColumnType::Time32,
    ] {
        assert_eq!(column.takes_float(), Ok(()));
    }
}
