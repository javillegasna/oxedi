//! Counts, sums and the payer and payee of a parse, read from its tables.

use std::collections::BTreeSet;

use edi835_core::{Cell, ColumnData, ColumnType, Table, Tables};
use pyo3::exceptions::{PyKeyError, PyTypeError, PyValueError};
use pyo3::prelude::*;

/// The table `name`, or a `KeyError` that names the method and the tables.
pub fn table<'t>(tables: &'t Tables, method: &str, name: &str) -> PyResult<&'t Table> {
    tables.get(name).ok_or_else(|| {
        let names: Vec<&str> = tables.iter().map(Table::name).collect();
        PyKeyError::new_err(format!(
            "Result.{method} reads the table \"{name}\"; the tables are: {}",
            names.join(", ")
        ))
    })
}

/// The column `name` of `table`, or a `KeyError` that names its columns.
pub fn column<'t>(table: &'t Table, method: &str, name: &str) -> PyResult<&'t ColumnData> {
    table.column(name).ok_or_else(|| {
        let names: Vec<&str> = table
            .columns()
            .iter()
            .map(|(column, _)| column.as_str())
            .collect();
        PyKeyError::new_err(format!(
            "Result.{method} reads the column \"{name}\" of the table \"{}\"; its columns are: {}",
            table.name(),
            names.join(", ")
        ))
    })
}

/// Rows of `claims`.
pub fn count_claims(tables: &Tables) -> PyResult<usize> {
    Ok(table(tables, "count_claims", "claims")?.len())
}

/// Distinct non-null `claims.patient_id` values, compared as text: no
/// numeric normalization, so `0123` and `123` are two patients.
pub fn count_patients(tables: &Tables) -> PyResult<usize> {
    let claims = table(tables, "count_patients", "claims")?;
    let ids = column(claims, "count_patients", "patient_id")?;
    let mut seen: BTreeSet<Vec<u8>> = BTreeSet::new();
    for row in 0..ids.len() {
        match ids.get(row) {
            Some(Cell::Binary(bytes)) => {
                seen.insert(bytes.to_vec());
            }
            Some(Cell::Null) | None => {}
            Some(_) => {
                seen.insert(ids.render(row).unwrap_or_default().into_bytes());
            }
        }
    }
    Ok(seen.len())
}

/// The sum of `payments.total_payment_amount` as fixed-point text at the
/// column's scale; nulls are skipped.
pub fn sum_payments(tables: &Tables) -> PyResult<String> {
    let payments = table(tables, "sum_payments", "payments")?;
    let amounts = column(payments, "sum_payments", "total_payment_amount")?;
    let ColumnType::Decimal128 { scale, .. } = amounts.kind() else {
        return Err(PyTypeError::new_err(format!(
            "Result.sum_payments reads \"total_payment_amount\" of the table \"payments\" as a decimal; it is {}",
            amounts.kind()
        )));
    };
    let mut sum: i128 = 0;
    for row in 0..amounts.len() {
        if let Some(Cell::Decimal128(value)) = amounts.get(row) {
            sum = sum.checked_add(value).ok_or_else(|| {
                PyValueError::new_err(format!(
                    "Result.sum_payments: the sum of \"total_payment_amount\" in the table \"payments\" overflows a 128-bit decimal at row {row} (amount {})",
                    fixed_point(value, scale)
                ))
            })?;
        }
    }
    Ok(fixed_point(sum, scale))
}

/// `value / 10^scale` written with exactly `scale` decimals and a leading
/// `-` when negative.
fn fixed_point(value: i128, scale: u8) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let mut digits = value.unsigned_abs().to_string();
    let scale = usize::from(scale);
    if scale == 0 {
        return format!("{sign}{digits}");
    }
    if digits.len() <= scale {
        digits = format!("{}{digits}", "0".repeat(scale + 1 - digits.len()));
    }
    let (whole, decimals) = digits
        .split_at_checked(digits.len() - scale)
        .unwrap_or((&digits, ""));
    format!("{sign}{whole}.{decimals}")
}

/// Which organization: the column prefix and the name used in messages.
#[derive(Clone, Copy)]
pub enum Role {
    Payer,
    Payee,
}

impl Role {
    fn name(self) -> &'static str {
        match self {
            Role::Payer => "payer",
            Role::Payee => "payee",
        }
    }
}

/// The key of the Python dict and the `payments` column suffix of each field.
const FIELDS: [(&str, &str); 6] = [
    ("name", "name"),
    ("identification_code", "id"),
    ("address", "address"),
    ("city", "city"),
    ("state", "state"),
    ("zip_code", "zip"),
];

/// One organization field per key: `None` for a null cell.
pub type Organization = [(&'static str, Option<String>); 6];

/// The one `payments` row's `<role>_` columns as text; `None` when every one
/// is null (the file has no such loop); a `ValueError` when `payments` has
/// other than one row.
pub fn organization(tables: &Tables, role: Role) -> PyResult<Option<Organization>> {
    let method = role.name();
    let payments = table(tables, method, "payments")?;
    if payments.len() != 1 {
        return Err(PyValueError::new_err(format!(
            "Result.{method} reads one transaction; the table \"payments\" has {} rows",
            payments.len()
        )));
    }
    let mut found: Organization = FIELDS.map(|(key, _)| (key, None));
    for (slot, (_, suffix)) in found.iter_mut().zip(FIELDS) {
        let data = column(payments, method, &format!("{method}_{suffix}"))?;
        slot.1 = match data.get(0) {
            None | Some(Cell::Null) => None,
            Some(_) => data.render(0),
        };
    }
    if found.iter().all(|(_, value)| value.is_none()) {
        return Ok(None);
    }
    Ok(Some(found))
}
