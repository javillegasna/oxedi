# Writing 835 files

`oxedi.write` turns tables back into an 835: the tables of a parse, or your own Arrow, Polars
or pandas tables with the spec's columns. To reproduce a parsed file byte for byte, use
`result.document.write()` instead (see [Reading](reading.md#the-document-and-writing-the-file-back)).

## Parse, change, write

```python
import datetime
import polars as pl
import oxedi

result = oxedi.parse_file("remittance.835")
frames = {name: result.tables[name].to_polars() for name in result.tables.keys()}
frames["payments"] = frames["payments"].with_columns(trace_number=pl.lit("CHK100235"))

envelope = oxedi.Envelope(
    sender_id="ACMEPAYER",
    receiver_id="SUNRISECLINIC",
    date=datetime.date(2024, 1, 10),
    time=datetime.time(9, 0),
    usage_indicator="T",  # "P" (production) is the default
)
data = oxedi.write(frames, envelope, spec=result.spec)  # bytes, one interchange
```

## Which spec writes

The spec is the one that parsed the tables, else the built-in 5010 spec. A dictionary of
frames, as above, does not carry the parse's spec, so pass it: `spec=result.spec`
(`result.tables.spec` is the same one). For a 4010 file you can also pass
`oxedi.Spec.builtin("4010")`. Without it a 4010 file is written as 5010 and refused.

## What is written

A table left out has no rows and a column left out is null. Rows nest by their `payment`,
`claim` and `service` columns, the row number of their parent.

Only what the tables hold is written: segments no column reads (for example the payer's
`PER*CX` or the bank details of `BPR`) are left out. The transaction set header carries no
implementation convention reference, so a written 5010 header is `ST*835*0001~`.

## Envelope

`oxedi.Envelope` gives what the tables cannot:

- `sender_id` and `receiver_id`, with their qualifiers `sender_qualifier` and
  `receiver_qualifier` (default `"ZZ"`).
- `date` and `time`.
- `usage_indicator` (`"P"` by default).
- `control_number`, the first one (default 1).
- `application_sender` and `application_receiver`, optional, for the group header.
- `delimiters`: default `*`, `:`, `~` and repetition `^`.
- `line_break`: a line break after each segment.

The writer works out the rest: the fixed-width `ISA`, the control numbers that must match
(`ISA13`/`IEA02`, `GS06`/`GE02`, `ST02`/`SE02`) and the counts (`SE01`, `GE01`, `IEA01`).

## Findings

The writer is strict by default. The written file is read back with the spec, and every
diagnostic of that read is a finding, as is anything the writer cannot place: for example a
required element or occurrence without a value, a value outside its code list or length, a row
whose parent does not exist or that comes out of order, a value holding a delimiter, or money
that does not balance. A value holding a delimiter is reported first: the file then splits
where the data does not, so it is not read back, and the other findings show once that value is
fixed.

Any finding raises `oxedi.WriteError`, a `ValueError` whose message lists every finding and
whose `findings` holds them as `oxedi.WriteFinding` (each names the table, row and column, or
the envelope field, with the diagnostic behind it). Nothing is written. With
`allow_findings=True` the call returns `(data, findings)` instead, for example to produce
invalid files for tests:

```python
try:
    data = oxedi.write(frames, envelope, spec=result.spec)
except oxedi.WriteError as error:
    for finding in error.findings:
        print(finding.table, finding.row, finding.column, finding)

data, findings = oxedi.write(frames, envelope, spec=result.spec, allow_findings=True)
```

## Money

The writer never changes money. The balancing rules are checked, never used to fill a total:

- `BPR02` against the claims' payments and the `PLB` adjustments.
- Each claim's and each service's charge minus payment against their `CAS` adjustments.

The claim rule does not model interest (`AMT*I`): a claim whose payment includes interest does
not balance and is refused unless `allow_findings` is set.

## Null and empty text

In text columns, `null` is an element the segment does not have and `""` one it has but leaves
empty. A null cell in the middle of a segment is still written as an empty element, so it
reads back as `""`; only trailing nulls read back as `null`. The tables of a parse write back
to the same tables.

## From DuckDB

The `oxedi` extension writes the same file from SQL, byte for byte equal to `oxedi.write` with
the same envelope. See [DuckDB](duckdb.md#writing).

## From your own tables

`oxedi.write` takes the spec's table and column names and nothing else.

- **Tolerated:** a missing table (no rows), a missing column (null) and a column of a narrower
  or compatible type (an integer for a big integer, a decimal of another scale that holds the
  value).
- **Refused, naming the table, row and column:** a table or column that is not in the spec,
  floats for money, a value that would change in the conversion, and a file whose totals do
  not balance or that lacks a required element.

The spec's tables key their rows by position: `row` counts the rows of a table from 0, and
`payment`, `claim` and `service` hold the `row` of the parent. If your tables have their own
keys, number the rows, bring the parents' `row` in with a join and drop your helper columns.
Rows of one parent must be together and in their parents' order, so sort each child table by
the parent's `row` first, then by its own key (keep the table's order for ties). The
adjustments of a claim (no `service`) come before those of its services. Keep the `segment`
column of `provider_adjustments` when you have one: adjustments with the same `segment` are
written in one `PLB`, and without it all the adjustments of a payment share one `PLB`.

To see a table's columns, use `result.tables["claims"].to_polars().schema` (or `.columns`) on
any parsed file, or in the extension
`DESCRIBE SELECT * FROM read_835('remittance.835', table_name := 'claims')`. A schema that
does not need a file is tracked in [#138](https://github.com/javillegasna/oxedi/issues/138).

### With Polars

Use `with_row_index("row")` to number the rows.

```python
import polars as pl

# my_payments (pay_key), my_claims (claim_key, pay_key), my_services (service_key,
# claim_key), my_adjustments (claim_key, service_key or null), my_provider_adjustments
# (pay_key): your tables, each with the spec's other columns
payments = my_payments.sort("pay_key").with_row_index("row")
claims = (
    my_claims.join(payments.select("row", "pay_key"), on="pay_key", maintain_order="left")
    .rename({"row": "payment"})
    .sort("payment", "claim_key", maintain_order=True)
    .with_row_index("row")
)
claim_rows = claims.select(claim="row", payment="payment", claim_key="claim_key")
services = (
    my_services.join(claim_rows, on="claim_key", maintain_order="left")
    .sort("claim", "service_key", maintain_order=True)
    .with_row_index("row")
)
adjustments = (
    my_adjustments.join(claim_rows, on="claim_key", maintain_order="left")
    .join(
        services.select(service="row", service_key="service_key"),
        on="service_key", how="left", maintain_order="left",
    )
    .sort("claim", "service", nulls_last=False, maintain_order=True)
    .with_row_index("row")
)
provider_adjustments = (
    my_provider_adjustments.join(
        payments.select(payment="row", pay_key="pay_key"), on="pay_key", maintain_order="left"
    )
    .sort("payment", maintain_order=True)
    .with_row_index("row")
)
frames = {
    "payments": payments.drop("pay_key"),
    "claims": claims.drop("pay_key", "claim_key"),
    "services": services.drop("service_key", "claim_key"),
    "adjustments": adjustments.drop("claim_key", "service_key"),
    "provider_adjustments": provider_adjustments.drop("pay_key"),
}
data = oxedi.write(frames, envelope)
```

### With SQL (DuckDB)

Give each table a `row` with `row_number()`, put the parents' `row` in its parent columns with a
join, and drop your helper columns with `EXCLUDE`. Number each child table ordered by the
parent's `row` first, then by its own key, then by a stable tie-breaker (a sequence column, or
`rowid` of a table). The adjustments of a claim, which have no `service`, come before those of
its services (`NULLS FIRST`). Your keys need not sort in parent order across parents.

This example takes `my_payments` (key `pay_key`), `my_claims` (`claim_key`, `pay_key`),
`my_services` (`service_key`, `claim_key`), `my_adjustments` (`claim_key`, and `service_key`
when the adjustment is on a service) and `my_provider_adjustments` (`pay_key`), each with the
spec's other columns:

```sql
COPY (
  WITH payments AS (
         SELECT row_number() OVER (ORDER BY pay_key) - 1 AS "row", * FROM my_payments),
       claims AS (
         SELECT row_number() OVER (ORDER BY p."row", c.claim_key) - 1 AS "row", p."row" AS payment, c.*
         FROM my_claims c JOIN payments p USING (pay_key)),
       services AS (
         SELECT row_number() OVER (ORDER BY c."row", s.service_key) - 1 AS "row",
                c.payment, c."row" AS claim, s.*
         FROM my_services s JOIN claims c USING (claim_key)),
       adjustments AS (
         SELECT row_number() OVER (ORDER BY c."row", s."row" NULLS FIRST, a.rowid) - 1 AS "row",
                c.payment, c."row" AS claim, s."row" AS service, a.*
         FROM my_adjustments a
         JOIN claims c USING (claim_key)
         LEFT JOIN services s ON s.service_key = a.service_key),
       provider_adjustments AS (
         SELECT row_number() OVER (ORDER BY p."row", a.rowid) - 1 AS "row", p."row" AS payment, a.*
         FROM my_provider_adjustments a JOIN payments p USING (pay_key))
  SELECT {
    'payments': (SELECT list(t ORDER BY t."row") FROM (SELECT * EXCLUDE (pay_key) FROM payments) t),
    'claims': (SELECT list(t ORDER BY t."row")
               FROM (SELECT * EXCLUDE (pay_key, claim_key) FROM claims) t),
    'services': (SELECT list(t ORDER BY t."row")
                 FROM (SELECT * EXCLUDE (service_key, claim_key) FROM services) t),
    'adjustments': (SELECT list(t ORDER BY t."row")
                    FROM (SELECT * EXCLUDE (claim_key, service_key) FROM adjustments) t),
    'provider_adjustments': (SELECT list(t ORDER BY t."row")
                             FROM (SELECT * EXCLUDE (pay_key) FROM provider_adjustments) t)
  }
) TO 'out.835' (FORMAT edi835, sender_id 'ACMEPAYER', receiver_id 'SUNRISECLINIC',
                date DATE '2024-01-10', time TIME '09:00');
```
