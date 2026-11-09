# Sharing data without patient information

An 835 names the patient: name, member id, claim number and service dates. Pass a mask to
oxedi and it replaces those values in the tables, and in the diagnostics that quote them, with
tokens and coarser dates. You can then share the tables, attach them to a ticket or build
synthetic test files from them.

```python
import oxedi

mask = oxedi.Mask("a-secret-of-at-least-16-bytes", dates="year")
result = oxedi.parse_file("remittance.835", mask=mask)
```

## What is masked

The spec flags the columns that hold patient data, so the list below is the built-in spec's. A
patched spec can flag others (see [Customizing the spec](../customize/spec.md#flag-patient-data)).

| Table | Column | Kind |
|---|---|---|
| `claims` | `patient_last_name`, `patient_first_name` | name |
| `claims` | `patient_id`, `claim_id`, `payer_control_number`, `group_number` | id |
| `claims` | `statement_from`, `statement_to` | date |
| `services` | `line_item_control_number` | id |
| `services` | `service_date` | date |
| `provider_adjustments` | `reference_id` | id |

A name or an id becomes 16 lowercase hexadecimal characters: the first 8 bytes of an HMAC-SHA256
of the message `kind`, a zero byte, then the value (`kind` is `name` or `id`), keyed with your key,
so a token can be reproduced from the key. Two consequences follow:

- The same value and key always give the same token, so a masked `patient_id` still joins the
  same patient across claims and files.
- Without the key the original cannot be recovered from a token.

An empty value stays empty, and a missing value stays null.

The diagnostics are masked too. The value a finding quotes is masked when it comes from `DTM02`,
`REF02`, `NM1` elements 3, 4 and 9, `CLP01`, `CLP07` or the reference of a `PLB` adjustment
(the second component of each adjustment identifier, read into
`provider_adjustments.reference_id`). This covers every segment of those types,
so it also hides the value of a finding about the payer, the payee or a provider, such as one
about a `DTM*405` date or a payer `REF02`. Amounts a finding quotes stay as they are.
A `LengthOutOfRange` finding still states the length of the original value. With
`dates="shift"`, a date inside a diagnostic is reduced to its year.

## Choose what happens to dates

| `dates` | Result |
|---|---|
| `"year"` (default) | January 1 of the same year. |
| `"shift"` | A claim and its service lines move by the same number of days, between -182 and +182, derived from the key and the claim's patient id. Intervals inside a claim are kept. |
| `"keep"` | The date is unchanged. |

A claim with no patient id cannot be shifted: its dates and those of its service lines fall back
to `"year"`.

```python
import polars as pl

key = "a-secret-of-at-least-16-bytes"
for dates in ("year", "shift"):
    result = oxedi.parse_file("remittance.835", mask=oxedi.Mask(key, dates=dates))
    claims = result.tables.to_polars()["claims"]
    print(dates, claims.select(pl.col("patient_last_name").cast(pl.String), "statement_from").row(0))
```

```text
year ('2fc81fb06ff98c31', datetime.date(2024, 1, 1))
shift ('2fc81fb06ff98c31', datetime.date(2023, 9, 2))
```

## The key

The key is bytes or text of at least 16 bytes. A shorter one raises `ValueError`
("the mask key must be at least 16 bytes long; got 5"). There is no default key, so a mask
never exists by accident.

Keep the key out of the files you share, and reuse it when you need the same token for the same
patient in another run.

`oxedi.parse`, `oxedi.parse_file` and `oxedi.stream` all take `mask=` and mask the same way.

## Validate with pyx12

`oxedi.pyx12.validate` takes `mask=` too. A finding about a place the masked columns read gets
its value masked as `parse` masks its own diagnostics. In pyx12's message, that value and every
masked value of the segment the finding points at are replaced by their masked forms, also when
the finding names no element (pyx12's "Segment CLP*... not found" quotes the claim id). A value
of 4 characters or more is replaced wherever it occurs. A shorter one is replaced only where it
stands apart: the character on each side is the edge of the message, a character that is not an
ASCII letter or digit (an accented letter or `-` counts), or part of another replaced value. So a
name `D` leaves the word `Data` as written. Short values that overlap or touch are replaced as one
group when the group stands apart, so initials `D` and `J` in `*DJ*` are both replaced. A short
value glued to letters or digits that are not flagged values stays: with `XAB` and `ABYZ`
flagged, `QXABYZ` keeps `QX`. The rule can also mask too much: initials `A` and `N` replace the
`AN` of pyx12's "type AN". The
findings, their order and where they point stay the same as without the mask. `validate` takes
`spec=` too: it masks the places that spec flags (the built-in spec of the file's version when it
is omitted), so a column only a patched spec flags is masked by `validate(..., mask=, spec=patched)`
as by `parse(..., spec=patched, mask=)`; the same spec gives the findings' loop paths. This example
writes an invalid claim date into the example file:

```python
from pathlib import Path
from oxedi.pyx12 import validate

data = Path("remittance.835").read_bytes().replace(b"DTM*232*20240105", b"DTM*232*20240230")
for d in validate(data, mask=oxedi.Mask(key)):
    print(d.code, d.datum, d.rule)
```

```text
8 b'46891844d3db70db' Data element "Claim Date" (DTM02) contains an invalid date (46891844d3db70db) (reported by pyx12, code 8)
```

pyx12 still reads the original file: the mask hides what `validate` returns, not what pyx12 is
given. A pyx12 message about a length still states the length of the original value. When pyx12
raises an exception, its finding keeps the exception's type and leaves out its text, which could
quote any value of the file: `could not finish validating: ValueError, whose message is not shown
with a mask; ...`, or `its error tree could not be read (ValueError, whose message is not shown
with a mask)` when pyx12's error output cannot be read. The rest of that finding, which says which
segment pyx12 was processing, is oxedi's own text and is never rewritten, so its segment numbers
stay readable; its datum is masked like any other. The finding for a file pyx12 refuses to read
quotes pyx12's own error text, which can hold the ISA line, and is masked like any pyx12 message; its datum is `ISA`, not the ISA line.

## Use it in SQL

`read_835` takes `mask_key` and `mask_dates` (`'year'`, `'shift'` or `'keep'`). A NULL key is
refused, and `mask_dates` without `mask_key` is an error.

```sql
SET VARIABLE mask_key = 'a-secret-of-at-least-16-bytes';

SELECT patient_last_name, statement_from
FROM read_835('remittance.835', mask_key := getvariable('mask_key'), mask_dates := 'year')
LIMIT 2;
```

```text
┌───────────────────┬────────────────┐
│ patient_last_name │ statement_from │
│      varchar      │      date      │
├───────────────────┼────────────────┤
│ 2fc81fb06ff98c31  │ 2024-01-01     │
│ 924149a1fc712857  │ 2024-01-01     │
└───────────────────┴────────────────┘
```

!!! warning "A literal key stays in the query history"
    A key written in the SQL text can appear in `EXPLAIN ANALYZE` output, in the `LINE 1:`
    excerpt of an error message and in your client's query history. `SET VARIABLE` with a
    literal, as above, keeps it in the history too. Pass the key as a prepared-statement
    parameter instead, so it never enters the SQL text:

    <!-- docs-test: skip needs the DuckDB extension loaded in the connection -->
    ```python
    import duckdb

    con = duckdb.connect()                    # with the oxedi extension loaded
    rows = con.execute(
        "SELECT patient_last_name FROM read_835(?, mask_key := ?, mask_dates := ?)",
        ["remittance.835", key, "shift"],
    ).fetchall()
    ```

## What is not masked

- **The raw file.** `result.document` and every `Segment` keep the original bytes, because oxedi
  never changes them. Do not share `result.document.write()`.
- **Payer, payee and provider data.** Names, addresses and identification codes of the payer, the
  payee and the rendering provider are not patient data and stay as they are.

With the default `dates="year"`, this follows the HIPAA Safe Harbor identifiers that appear in the
tables: names, member and claim numbers, and dates reduced to the year. `"shift"` and `"keep"`
do not remove dates the way Safe Harbor requires. It is not a legal de-identification
determination. Free text, rare combinations of values and the data you add yourself are still
yours to review.

## Make synthetic test files

Masked tables are valid input for the writer, so you can turn a real file into a shareable one:

```python
import datetime
import oxedi

result = oxedi.parse_file("remittance.835", mask=oxedi.Mask(key))
envelope = oxedi.Envelope(
    sender_id="ACMEPAYER",
    receiver_id="SUNRISECLINIC",
    date=datetime.date(2024, 1, 10),
    time=datetime.time(9, 0),
    usage_indicator="T",
)
data = oxedi.write(result.tables.to_polars(), envelope, spec=result.spec)
print(oxedi.parse(data).diagnostics)
```

```text
[]
```

## In the browser

The [viewer](../start/viewer.md) has a **Hide patient data** switch that applies the same mask
to the tables, the diagnostics and the CSV export.
