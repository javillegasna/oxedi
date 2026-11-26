# Get started

In five minutes you will parse a remittance file, see what it paid, find one problem in it and
write it back byte for byte. You need Python 3.11 or later.

!!! tip "New to the 835?"
    An 835 is the file a health plan sends to a provider to explain a payment.
    [The 835 in five minutes](the-835.md) explains the format. You can read it before or after
    this page.

## Install oxedi

```bash
pip install "oxedi[polars]"
```

This installs oxedi with Polars, which the examples use to show tables. Plain
`pip install oxedi` has no Python dependencies at all. [Install](install.md) lists every extra
and platform.

## Get the example file

The examples read a small file named `remittance.835`. It holds one payment of 690.00 from
ACME HEALTH PLAN to SUNRISE CLINIC, covering four claims. Download it into your working
directory:

```bash
curl -o remittance.835 https://raw.githubusercontent.com/javillegasna/oxedi/master/crates/oxedi_core/tests/fixtures/balanced_5010_sample.txt
```

## Parse the file

```python
import oxedi

result = oxedi.parse_file("remittance.835")

print(result)
print(result.summary("count_claims"), result.summary("sum_payments"))
print(result.summary("payer")["name"], "paid", result.summary("payee")["name"])
```

```text
Result(segments=63, tables=5, diagnostics=0)
4 690.00
ACME HEALTH PLAN paid SUNRISE CLINIC
```

The file has 63 segments, the lines of an 835. oxedi read them into five tables and found no
problems. `summary("sum_payments")` is a `Decimal`, so money is exact.

## Look at the claims

The five tables are ready to use:

```python
print(result.tables)
```

```text
Tables(adjustments: 15 rows, claims: 4 rows, payments: 1 rows, provider_adjustments: 3 rows, services: 4 rows)
```

Open the claims in Polars:

```python
import polars as pl

claims = result.tables["claims"].to_polars().with_columns(pl.col(pl.Binary).cast(pl.String))
print(claims.select("row", "claim_id", "claim_status", "charge_amount", "payment_amount"))
```

```text
┌─────┬──────────┬──────────────┬───────────────┬────────────────┐
│ row ┆ claim_id ┆ claim_status ┆ charge_amount ┆ payment_amount │
│ --- ┆ ---      ┆ ---          ┆ ---           ┆ ---            │
│ i64 ┆ str      ┆ str          ┆ decimal[38,2] ┆ decimal[38,2]  │
╞═════╪══════════╪══════════════╪═══════════════╪════════════════╡
│ 0   ┆ CLM1001  ┆ 1            ┆ 300.00        ┆ 200.00         │
│ 1   ┆ CLM1002  ┆ 2            ┆ 700.00        ┆ 400.00         │
│ 2   ┆ CLM2001  ┆ 1            ┆ 150.00        ┆ 120.00         │
│ 3   ┆ CLM2002  ┆ 4            ┆ 80.00         ┆ 0.00           │
└─────┴──────────┴──────────────┴───────────────┴────────────────┘
```

- Each row is one claim the provider sent to the payer.
- `charge_amount` is what the provider billed. `payment_amount` is what the payer paid.
- Claim status `4` is a denial: CLM2002 billed 80.00 and got nothing.

!!! note "Why the cast to `String`?"
    oxedi keeps text exactly as the file has it, as raw bytes, so nothing is lost to an
    encoding guess. Cast to `String` when you want to read or group by text.

## See why a claim was paid less

CLM1001 billed 300.00 and was paid 200.00. The adjustments explain the difference. They link
to their claim through the `claim` column, which holds the claim's `row`:

```python
adjustments = result.tables["adjustments"].to_polars().with_columns(pl.col(pl.Binary).cast(pl.String))
print(adjustments.filter(pl.col("claim") == 0).select("service", "group_code", "reason_code", "amount"))
```

```text
┌─────────┬────────────┬─────────────┬───────────────┐
│ service ┆ group_code ┆ reason_code ┆ amount        │
│ ---     ┆ ---        ┆ ---         ┆ ---           │
│ i64     ┆ str        ┆ str         ┆ decimal[38,2] │
╞═════════╪════════════╪═════════════╪═══════════════╡
│ 0       ┆ PR         ┆ 1           ┆ 30.00         │
│ 0       ┆ CO         ┆ 45          ┆ 20.00         │
│ 1       ┆ PR         ┆ 2           ┆ 20.00         │
│ 1       ┆ PR         ┆ 3           ┆ 10.00         │
│ 1       ┆ CO         ┆ 45          ┆ 20.00         │
└─────────┴────────────┴─────────────┴───────────────┘
```

- The five adjustments add up to 100.00, the gap between 300.00 and 200.00.
- `group_code` says who bears the amount: `PR` is patient responsibility, `CO` a contractual
  obligation.
- `reason_code` is a claim adjustment reason code (CARC). See the
  [glossary](../reference/glossary.md#carc).
- `service` says which service line of the claim each adjustment belongs to.

[Tables and columns](../reference/tables.md) describes every table and column.

## Read a diagnostic

The example file is clean, so `result.diagnostics` is empty. To see a diagnostic, change one
claim status to `7`, a code the spec does not list, and parse again:

```python
from pathlib import Path

data = Path("remittance.835").read_bytes()
broken = oxedi.parse(data.replace(b"CLP*CLM1002*2*", b"CLP*CLM1002*7*"))

for diagnostic in broken.diagnostics:
    print(diagnostic)
```

```text
SNIP 2 · element CLP02 (claim_status_code) is not one of the 10 codes the spec lists for it · segment #36, element 2 · at interchange#1/group#1/transaction#1/2000#1/2100#2 · datum "7"
```

Read it from left to right:

- `SNIP 2`: the level of the rule. Level 2 covers required elements, types, lengths and codes.
- The rule that failed, with the element (`CLP02`, the claim status).
- Where: the segment at index 36 of the file (counting from 0), element 2.
- The loops open at that point: the second claim (`2100#2`) of the first transaction.
- `datum`: the value that broke the rule, `7`.

The parse did not stop. The claim is still in `broken.tables`, with its status `7`. To see the
segment itself, index the document with the diagnostic's segment:

```python
segment = broken.document[broken.diagnostics[0].segment]
print(segment.raw)
```

```text
b'\nCLP*CLM1002*7*700*400*0*12*PCN1002*11*1~'
```

[Diagnostics](../python/diagnostics.md) lists every kind of finding and what to do about it.

## Write the file back

oxedi keeps every byte of the file, so the document writes it back unchanged:

```python
assert result.document.write() == data
```

To build a new file from tables instead, use `oxedi.write`. [Writing](../python/writing.md)
shows how.

## Where next

- [The 835 in five minutes](the-835.md): what the file holds and how the tables map to it.
- [Reading 835 files](../python/reading.md): the tables in pandas, Polars and DuckDB.
- [Reading large files](../python/large-files.md): one transaction or one claim at a time.
- [DuckDB](../duckdb/index.md): the same tables from SQL, no Python needed.
- [Recipes](../recipes/lakehouse.md): a Parquet lakehouse, Polars analytics, pipelines.
