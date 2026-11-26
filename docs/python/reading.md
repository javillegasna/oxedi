# Reading 835 files

Read a file into tables, open them in your data library, and get the file back byte for byte.
For files too big to read at once, see [Large files](large-files.md){ #large-files }. For the problems a parse
reports, see [Diagnostics](diagnostics.md){ #diagnostics }.

## Parse a file

```python
import oxedi

result = oxedi.parse_file("remittance.835")   # or oxedi.parse(data) from bytes

print(result.summary("count_claims"), result.summary("sum_payments"))  # a sum is a Decimal
print(result.summary("payer")["name"], result.summary("payee")["name"])
```

```text
4 690.00
ACME HEALTH PLAN SUNRISE CLINIC
```

`result.summary("payer")` and `result.summary("payee")` are dicts of text values: `name`, `identification_code`,
`address`, `city`, `state` and `zip_code`.

!!! note "Files with more than one payment"
    `summary("payer")` and `summary("payee")` are `None` when the file has no such party. They raise
    `ValueError` when `payments` has more than one row. Read the `payer_*` and `payee_*`
    columns of `payments` instead.

!!! warning "A file that is not an 835"
    A file that is not an 835 at all raises `oxedi.ParseError`, for example one that does not
    start with an `ISA` segment. Anything else parses, and its problems come back as
    [diagnostics](diagnostics.md).

`parse`, `parse_file` and `stream` read the version the file declares and use the matching
built-in spec. To choose another one or extend it, see [the spec](../customize/spec.md).

To share a file's tables without the patient's name, ids and dates, pass `mask=` to any of them.
See [Sharing data without patient information](without-patient-data.md).

## Use the tables

Every parse gives the same five tables:

| Table | One row per | Linked by |
|---|---|---|
| `payments` | payment (transaction) | none |
| `claims` | claim | `payment` |
| `services` | service line | `payment`, `claim` |
| `adjustments` | claim or service adjustment | `payment`, `claim`, `service` |
| `provider_adjustments` | provider-level adjustment | `payment` |

- `result.tables.keys()` lists the tables. `table.columns` lists a table's columns.
- Amounts are exact decimals and dates are dates.
- Text is kept as the raw bytes of the file (Arrow `binary`), so nothing is lost to an encoding
  guess.

[Tables and columns](../reference/tables.md) describes every column and where it comes from.

### Open a table in your library

=== "Polars"

    ```python
    claims = result.tables["claims"].to_polars()
    print(claims.select("row", "claim_id", "charge_amount", "payment_amount").head(3))
    ```

    ```text
    ┌─────┬────────────┬───────────────┬────────────────┐
    │ row ┆ claim_id   ┆ charge_amount ┆ payment_amount │
    │ --- ┆ ---        ┆ ---           ┆ ---            │
    │ i64 ┆ binary     ┆ decimal[38,2] ┆ decimal[38,2]  │
    ╞═════╪════════════╪═══════════════╪════════════════╡
    │ 0   ┆ b"CLM1001" ┆ 300.00        ┆ 200.00         │
    │ 1   ┆ b"CLM1002" ┆ 700.00        ┆ 400.00         │
    │ 2   ┆ b"CLM2001" ┆ 150.00        ┆ 120.00         │
    └─────┴────────────┴───────────────┴────────────────┘
    ```

=== "pandas"

    ```python
    claims = result.tables["claims"].to_pandas()
    print(claims[["row", "claim_id", "charge_amount", "payment_amount"]].head(3))
    ```

    ```text
       row    claim_id charge_amount payment_amount
    0    0  b'CLM1001'        300.00         200.00
    1    1  b'CLM1002'        700.00         400.00
    2    2  b'CLM2001'        150.00         120.00
    ```

=== "DuckDB"

    ```python
    import duckdb

    claims = result.tables["claims"]
    print(duckdb.sql("select claim_status, sum(payment_amount) from claims group by 1 order by 1"))
    ```

    ```text
    ┌──────────────┬─────────────────────┐
    │ claim_status │ sum(payment_amount) │
    │     blob     │    decimal(38,2)    │
    ├──────────────┼─────────────────────┤
    │ 1            │              320.00 │
    │ 2            │              400.00 │
    │ 4            │                0.00 │
    └──────────────┴─────────────────────┘
    ```

`to_polars()` needs `oxedi[polars]` and `to_pandas()` needs `oxedi[pandas]`.

??? note "Details: any Arrow library reads the tables"
    Tables follow the Arrow PyCapsule interface. Any Arrow-aware library reads them directly,
    without copying the data:

    ```python
    import polars as pl

    pl.DataFrame(result.tables["services"])
    ```

### Join a child table to its parent

`row` counts the rows of a table from 0. `payment`, `claim` and `service` hold the `row` of the
parent. So a service joins to its claim on `services.claim = claims.row`:

```python
import polars as pl

claims = result.tables["claims"].to_polars()
services = result.tables["services"].to_polars()
lines = services.join(claims.select("row", "claim_id"), left_on="claim", right_on="row")
print(lines.select(pl.col("claim_id", "procedure_code").cast(pl.String), "payment_amount"))
```

```text
┌──────────┬────────────────┬────────────────┐
│ claim_id ┆ procedure_code ┆ payment_amount │
│ ---      ┆ ---            ┆ ---            │
│ str      ┆ str            ┆ decimal[38,2]  │
╞══════════╪════════════════╪════════════════╡
│ CLM1001  ┆ 99213          ┆ 150.00         │
│ CLM1001  ┆ 85025          ┆ 50.00          │
│ CLM1002  ┆ 97110          ┆ 450.00         │
│ CLM2001  ┆ 99212          ┆ 150.00         │
└──────────┴────────────────┴────────────────┘
```

## Get the file back byte for byte

Nothing is dropped: every byte of the file belongs to exactly one segment. The document keeps
them, so it returns the file exactly as it was read:

```python
from pathlib import Path

data = Path("remittance.835").read_bytes()
assert oxedi.parse(data).document.write() == data
```

`document.write()` reproduces the file. To produce a new file from tables, see
[Writing](writing.md).

??? note "Details: a file that starts with a byte order mark"
    A file that starts with a UTF-8 byte order mark is still read. The mark stays in the first
    segment, `write()` gives the file back unchanged, and a `ByteOrderMark` diagnostic says it
    was there.
