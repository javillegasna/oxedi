# oxedi

A fast EDI 835 (electronic remittance advice) parser for Python, written in Rust.

- **Lossless.** Every byte of the file is kept, so `write()` returns the file exactly as it
  was read.
- **Ready-made tables.** Payments, claims, service lines and adjustments come out as typed
  tables that open in Polars, pandas, pyarrow or DuckDB without copying the data.
- **Problems are reported, not raised.** A file that is valid EDI but has questionable data
  still parses; each issue is returned as a diagnostic that names the rule, the position and
  the value.
- **Large files.** `stream` yields one transaction at a time, so memory stays bounded.
- **Extensible.** Add segments or table columns with a small JSON patch, without touching
  the code.

## Install

```bash
pip install oxedi               # no Python dependencies
pip install "oxedi[polars]"     # with Polars
pip install "oxedi[pandas]"     # with pandas and pyarrow
```

Requires Python 3.11 or later. Wheels are available for Linux (x86_64 and aarch64, plus
musl on x86_64), macOS (Intel and Apple silicon) and Windows (x86_64).

## Quick start

```python
import oxedi

result = oxedi.parse_file("remittance.835")

claims = result.tables["claims"].to_polars()      # or .to_pandas()
services = result.tables["services"].to_polars()

print(result.count_claims(), result.sum_payments())  # sum_payments() is a Decimal
print(result.payer["name"], result.payee["name"])

for diagnostic in result.diagnostics:
    print(diagnostic)
```

`oxedi.parse(data)` does the same from `bytes`. A file that is not an 835 at all (for
example, one that does not start with an `ISA` segment) raises `oxedi.ParseError`.

### Tables

| Table | One row per | Linked by |
|---|---|---|
| `payments` | payment (transaction) | — |
| `claims` | claim | `payment` |
| `services` | service line | `payment`, `claim` |
| `adjustments` | claim or service adjustment | `payment`, `claim`, `service` |
| `provider_adjustments` | provider-level adjustment | `payment` |

`result.tables.keys()` lists them and `table.columns` lists a table's columns. Amounts are
decimals, dates are dates, and text fields are kept as the raw bytes from the file (Arrow
`binary`), so nothing is lost to an encoding guess. Tables follow
the Arrow PyCapsule interface, so any Arrow-aware library reads them directly:

```python
import duckdb, polars as pl

claims = result.tables["claims"]
duckdb.sql("select claim_status, sum(payment_amount) from claims group by 1")
pl.DataFrame(result.tables["services"])
```

### Large files

```python
from pathlib import Path

for batch in oxedi.stream(Path("big.835").read_bytes()):
    services = batch.tables["services"].to_polars()
```

Each batch holds the tables and diagnostics of one transaction. Pass `by="2100"` (or any
other loop id) to get one batch per claim instead. Parsing releases the GIL, so several
files can be parsed in parallel from threads.

### Writing the file back

```python
data = Path("remittance.835").read_bytes()
assert oxedi.parse(data).document.write() == data
```

## Extending the spec

The structure of the 835 and the columns of each table are defined by a JSON spec. A patch
in JSON Merge Patch format (RFC 7386) adapts it to a payer's variations:

```python
spec = oxedi.Spec.builtin().patch({
    "tables": {"claims": {"columns": {
        "contract_class": {"segment": "REF", "where": {"1": "CE"}, "element": 2}
    }}}
})
result = oxedi.parse(data, spec=spec)   # claims now has a contract_class column
```

Objects merge key by key, while arrays are replaced whole: to allow an extra segment in a
loop, list the loop's full `segments`:

```json
{"loops": {"1000A": {"segments": ["N3", "N4", "REF", "PER", "XX"]}}}
```

`parse` and `stream` both accept `spec=`.

## Spec versions

`parse`, `parse_file` and `stream` read the version a file declares (ISA12 and GS08) and use
the matching built-in spec: 5010 (`005010X221A1`) by default, 4010 (`004010X091A1`) for 4010
files. Pass `spec=` to override, or pick one yourself:

```python
spec = oxedi.Spec.builtin(version="4010")
```

## Validating with pyx12

`pip install "oxedi[pyx12]"` adds [pyx12](https://github.com/azoner/pyx12)'s
implementation-guide validation. `parse` never calls it; `validate` does, on demand:

```python
import oxedi
from oxedi.pyx12 import validate

result = oxedi.parse(data)
findings = result.diagnostics + validate(data)    # bytes, a path or a binary file object
for d in sorted(findings, key=lambda d: d.level):
    print(d.origin, d.code, d)                    # "oxedi" or "pyx12"; pyx12's own code
    if d.segment is not None:
        start, end = result.document[d.segment].span  # the segment's bytes in your file
```

`validate` returns `oxedi.Diagnostic`, the type `parse` returns, so the two lists mix, sort
by `level` and filter by `origin`. A pyx12 finding has `kind == "External"`,
`origin == "pyx12"`, `code` set to pyx12's error code and an empty `path` (pyx12 does not report
the loop); interchange, group and transaction findings are level 1, segment and element
findings level 2. A file pyx12 cannot read, or an exception inside pyx12, comes back as one
level 1 finding with no `code` whose `rule` starts with `could not finish validating`,
instead of a traceback.

## Coming from another library

Coming from edi-835-parser? See the [migration guide](https://github.com/javillegasna/oxedi/blob/master/docs/migrating-from-edi-835-parser.md).

## Versioning

The project follows semantic versioning. While the version is `0.x`, a minor release may change the API. See the [changelog](https://github.com/javillegasna/oxedi/blob/master/CHANGELOG.md).

## License

MIT. See [`LICENSE`](https://github.com/javillegasna/oxedi/blob/master/LICENSE) and [`THIRD_PARTY_NOTICES`](https://github.com/javillegasna/oxedi/blob/master/THIRD_PARTY_NOTICES).
