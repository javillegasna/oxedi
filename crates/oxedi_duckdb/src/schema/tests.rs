use edi835_core::ColumnType;

use super::SqlType;

fn of(kind: ColumnType, binary: bool) -> Result<SqlType, String> {
    SqlType::of("t", "c", kind, binary).map_err(|error| error.to_string())
}

#[test]
fn text_is_varchar_unless_binary() {
    assert_eq!(of(ColumnType::Binary, false), Ok(SqlType::Varchar));
    assert_eq!(of(ColumnType::Binary, true), Ok(SqlType::Blob));
}

#[test]
fn integers_are_bigint_whatever_their_scale() {
    assert_eq!(
        of(ColumnType::Int64 { scale: 0 }, false),
        Ok(SqlType::Bigint)
    );
    assert_eq!(
        of(ColumnType::Int64 { scale: 2 }, true),
        Ok(SqlType::Bigint)
    );
}

#[test]
fn decimals_keep_width_and_scale() {
    assert_eq!(
        of(
            ColumnType::Decimal128 {
                precision: 38,
                scale: 2
            },
            false
        ),
        Ok(SqlType::Decimal {
            width: 38,
            scale: 2
        })
    );
}

#[test]
fn a_decimal_not_stored_as_hugeint_is_refused() {
    assert_eq!(
        of(
            ColumnType::Decimal128 {
                precision: 18,
                scale: 2
            },
            false
        ),
        Err("read_835: table \"t\", column \"c\": the core type decimal128(18, 2) has no DuckDB type".to_owned())
    );
}

#[test]
fn dates_and_times() {
    assert_eq!(of(ColumnType::Date32, false), Ok(SqlType::Date));
    assert_eq!(of(ColumnType::Time32, false), Ok(SqlType::Time));
}
