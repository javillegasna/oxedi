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
