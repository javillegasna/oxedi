use libduckdb_sys as ffi;

use super::primitive_name;

#[test]
fn primitive_names_are_sql_names() {
    assert_eq!(
        primitive_name(ffi::DUCKDB_TYPE_DUCKDB_TYPE_INTEGER),
        "INTEGER"
    );
    assert_eq!(
        primitive_name(ffi::DUCKDB_TYPE_DUCKDB_TYPE_VARCHAR),
        "VARCHAR"
    );
    assert_eq!(
        primitive_name(ffi::DUCKDB_TYPE_DUCKDB_TYPE_BOOLEAN),
        "BOOLEAN"
    );
    assert_eq!(primitive_name(9999), "a type without a primitive name");
}
