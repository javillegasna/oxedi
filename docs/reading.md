# Reading 835 files

How to read a file into tables, handle large files, use the diagnostics and write the file
back byte for byte. The [README](../README.md) has the short version.

## Parse

```python
import oxedi

result = oxedi.parse_file("remittance.835")   # or oxedi.parse(data) from bytes

print(result.count_claims(), result.sum_payments())  # sum_payments() is a Decimal
print(result.payer["name"], result.payee["name"])
```

`result.payer` and `result.payee` are dicts of text values (`name`, `identification_code`,
`address`, `city`, `state`, `zip_code`). They are `None` when the file has no such party, and
raise `ValueError` when `payments` has more than one row.

A file that is not an 835 at all (for example, one that does not start with an `ISA`
segment) raises `oxedi.ParseError`. Anything else that parses is returned, with its problems
as diagnostics.

`parse`, `parse_file` and `stream` read the version a file declares and use the matching
built-in spec. See [Spec](spec.md) to choose another one or to extend it.

## Tables

| Table | One row per | Linked by |
|---|---|---|
| `payments` | payment (transaction) | none |
| `claims` | claim | `payment` |
| `services` | service line | `payment`, `claim` |
| `adjustments` | claim or service adjustment | `payment`, `claim`, `service` |
| `provider_adjustments` | provider-level adjustment | `payment` |

`result.tables.keys()` lists them and `table.columns` lists a table's columns. Amounts are
decimals, dates are dates, and text fields are kept as the raw bytes from the file (Arrow
`binary`), so nothing is lost to an encoding guess.

Each table converts to the library you use:

```python
claims = result.tables["claims"].to_polars()      # or .to_pandas()
```

Tables follow the Arrow PyCapsule interface, so any Arrow-aware library reads them directly,
without copying the data:

```python
import duckdb, polars as pl

claims = result.tables["claims"]
duckdb.sql("select claim_status, sum(payment_amount) from claims group by 1")
pl.DataFrame(result.tables["services"])
```

`row` counts the rows of a table from 0. `payment`, `claim` and `service` hold the `row` of
the parent, so a service joins to its claim on `services.claim = claims.row`.

## Large files

```python
from pathlib import Path

for batch in oxedi.stream(Path("big.835").read_bytes()):
    services = batch.tables["services"].to_polars()
```

Each batch holds the tables and diagnostics of one transaction. Pass `by="2100"` (or any
other loop id) to get one batch per claim instead. Parsing releases the GIL, so several files
can be parsed in parallel from threads. `stream` accepts `spec=` like `parse`.

## Diagnostics

A file that is valid EDI but has questionable data still parses. Each issue is returned in
`result.diagnostics` (and in each batch of `stream`) instead of being raised:

```python
for diagnostic in result.diagnostics:
    print(diagnostic)
```

A diagnostic names the rule that failed, where it failed and the offending value. Its fields:

- `level`: the SNIP level of the rule, 1, 2 or 3; sort by it to read the lowest first.
- `kind`: the name of the rule that failed, for example `RequiredElementMissing`.
- `rule`: the rule as a sentence with its values.
- `segment`: the index of the segment at fault, or `None` at the end of the file.
- `element` and `component`: the 1-based position inside the segment, when the finding has one.
- `path`: the loops open at that segment, outermost first, joined by `/`; empty at the root.
- `datum`: the offending value as it appears in the file (bytes).
- `origin` and `code`: who reported it (`"oxedi"`, or `"pyx12"` with pyx12's own code; see
  [Validating with pyx12](pyx12.md)).

To find the bytes of the segment a diagnostic points at, use the document (next section):
`result.document[d.segment].span` is the segment's byte range in your file.

## The document and writing the file back

Nothing is dropped: every byte of the file belongs to exactly one segment. The document keeps
them, so it returns the file exactly as it was read:

```python
data = Path("remittance.835").read_bytes()
assert oxedi.parse(data).document.write() == data
```

A file that starts with a UTF-8 byte order mark is still read: the mark stays in the first
segment, `write()` gives the file back unchanged, and a `ByteOrderMark` diagnostic says it was
there.

`document.write()` reproduces the file. To produce a new file from tables, see
[Writing](writing.md).
