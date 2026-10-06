//! Owned DuckDB values (parameters and settings) read through the C API.

use std::ffi::CStr;

use libduckdb_sys as ffi;

/// A DuckDB value the extension owns, destroyed on drop.
#[derive(Debug)]
pub struct Value(ffi::duckdb_value);

impl Value {
    /// Takes ownership of `raw`; `None` for a null handle or an SQL NULL.
    ///
    /// # Safety
    ///
    /// `raw` must be null or a value handle the caller owns and gives up.
    pub unsafe fn owned(raw: ffi::duckdb_value) -> Option<Value> {
        if raw.is_null() {
            return None;
        }
        let value = Value(raw);
        // SAFETY: `value.0` is a live value handle.
        let null = unsafe { ffi::duckdb_is_null_value(value.0) };
        (!null).then_some(value)
    }

    /// The raw handle, valid while `self` lives.
    pub fn raw(&self) -> ffi::duckdb_value {
        self.0
    }

    /// The value's type id.
    pub fn type_id(&self) -> ffi::DUCKDB_TYPE {
        // SAFETY: the value is live; the type it returns is borrowed from
        // the value and not destroyed here.
        unsafe { ffi::duckdb_get_type_id(ffi::duckdb_get_value_type(self.0)) }
    }

    /// The value's SQL type name, as DuckDB writes it (`INTEGER`,
    /// `INTEGER[]`).
    pub fn type_name(&self) -> String {
        // SAFETY: the value is live; its type is borrowed, not destroyed.
        unsafe { type_name(ffi::duckdb_get_value_type(self.0)) }
    }

    /// The value cast to text.
    pub fn text(&self) -> String {
        // SAFETY: the value is live; DuckDB returns a new string (or null)
        // that is freed below with `duckdb_free`.
        unsafe {
            let raw = ffi::duckdb_get_varchar(self.0);
            if raw.is_null() {
                return String::new();
            }
            let text = CStr::from_ptr(raw).to_string_lossy().into_owned();
            ffi::duckdb_free(raw.cast());
            text
        }
    }

    /// The value cast to a boolean.
    pub fn boolean(&self) -> bool {
        // SAFETY: the value is live.
        unsafe { ffi::duckdb_get_bool(self.0) }
    }

    /// The children of a LIST value, each `None` when NULL; empty for any
    /// other type.
    pub fn list(&self) -> Vec<Option<Value>> {
        if self.type_id() != ffi::DUCKDB_TYPE_DUCKDB_TYPE_LIST {
            return Vec::new();
        }
        // SAFETY: the value is a live LIST.
        let size = unsafe { ffi::duckdb_get_list_size(self.0) };
        (0..size)
            // SAFETY: `index` is below the list's size; the child returned is
            // owned by the caller, and `owned` takes it.
            .map(|index| unsafe { Value::owned(ffi::duckdb_get_list_child(self.0, index)) })
            .collect()
    }
}

impl Drop for Value {
    fn drop(&mut self) {
        // SAFETY: the handle is owned and destroyed once.
        unsafe { ffi::duckdb_destroy_value(&mut self.0) };
    }
}

/// The SQL name of a logical type: the primitive name, `<child>[]` for a
/// LIST, else the type's kind.
///
/// # Safety
///
/// `logical_type` must be a live logical type handle.
unsafe fn type_name(logical_type: ffi::duckdb_logical_type) -> String {
    // SAFETY: the type is live (caller contract).
    let id = unsafe { ffi::duckdb_get_type_id(logical_type) };
    if id == ffi::DUCKDB_TYPE_DUCKDB_TYPE_LIST {
        // SAFETY: the type is a live LIST; the child type is owned here and
        // destroyed after use.
        return unsafe {
            let mut child = ffi::duckdb_list_type_child_type(logical_type);
            let name = type_name(child);
            ffi::duckdb_destroy_logical_type(&mut child);
            format!("{name}[]")
        };
    }
    primitive_name(id).to_owned()
}

/// The SQL name of a type id.
pub fn primitive_name(id: ffi::DUCKDB_TYPE) -> &'static str {
    match id {
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_BOOLEAN => "BOOLEAN",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_TINYINT => "TINYINT",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_SMALLINT => "SMALLINT",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_INTEGER => "INTEGER",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_BIGINT => "BIGINT",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_HUGEINT => "HUGEINT",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_UTINYINT => "UTINYINT",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_USMALLINT => "USMALLINT",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_UINTEGER => "UINTEGER",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_UBIGINT => "UBIGINT",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_UHUGEINT => "UHUGEINT",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_FLOAT => "FLOAT",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_DOUBLE => "DOUBLE",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_DECIMAL => "DECIMAL",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_DATE => "DATE",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIME => "TIME",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIMESTAMP => "TIMESTAMP",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIMESTAMP_S => "TIMESTAMP_S",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIMESTAMP_MS => "TIMESTAMP_MS",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIMESTAMP_NS => "TIMESTAMP_NS",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIMESTAMP_TZ => "TIMESTAMP WITH TIME ZONE",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIME_TZ => "TIME WITH TIME ZONE",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_TIME_NS => "TIME_NS",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_INTERVAL => "INTERVAL",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_VARCHAR => "VARCHAR",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_BLOB => "BLOB",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_UUID => "UUID",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_STRUCT => "STRUCT",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_MAP => "MAP",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_ARRAY => "ARRAY",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_UNION => "UNION",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_ENUM => "ENUM",
        ffi::DUCKDB_TYPE_DUCKDB_TYPE_SQLNULL => "NULL",
        _ => "a type without a primitive name",
    }
}

#[cfg(test)]
mod tests;
