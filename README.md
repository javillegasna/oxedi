# oxedi

A fast, lossless parser and writer for X12 EDI 835 files (electronic remittance advice). The
core is written in Rust and is available from Python and from DuckDB.

- **Lossless.** Every byte of the file is kept. Nothing is silently dropped, and the file can
  be written back exactly as it was read.
- **Ready-made tables.** Payments, claims, service lines and adjustments come out as typed
  tables that open in Polars, pandas, pyarrow or DuckDB.
- **Problems are reported, not raised.** Questionable data still parses; each issue is a
  diagnostic that names the rule, the position and the value.

## Install

```bash
pip install oxedi               # no Python dependencies
pip install "oxedi[polars]"     # with Polars
pip install "oxedi[pandas]"     # with pandas and pyarrow
pip install "oxedi[pyx12]"      # with pyx12 validation
pip install "oxedi[edi-835-parser]"  # drop-in layer for edi-835-parser users
```

Requires Python 3.11 or later. Wheels are available for Linux (x86_64 and aarch64, plus musl
on x86_64), macOS (Intel and Apple silicon) and Windows (x86_64).

For DuckDB:

```sql
INSTALL oxedi FROM community;
LOAD oxedi;
```

The community build is not available yet: it waits for the maintainers to merge
duckdb/community-extensions#2942. Until then, build the extension and `LOAD` the file; see
[DuckDB](docs/duckdb.md).

## Read

```python
import oxedi

result = oxedi.parse_file("remittance.835")      # or oxedi.parse(data) from bytes

claims = result.tables["claims"].to_polars()     # or .to_pandas()
services = result.tables["services"].to_polars()

print(result.count_claims(), result.sum_payments())  # sum_payments() is a Decimal
print(result.payer["name"], result.payee["name"])
```

There are five tables: `payments`, `claims`, `services`, `adjustments` and
`provider_adjustments`. Child tables point to their parents by row number. Tables also follow
the Arrow PyCapsule interface, so DuckDB and any Arrow-aware library read them directly:

```python
import duckdb

claims = result.tables["claims"]
duckdb.sql("select claim_status, sum(payment_amount) from claims group by 1")
```

A file that is not an 835 at all raises `oxedi.ParseError`. Anything else parses, and its
problems come back as diagnostics:

```python
for diagnostic in result.diagnostics:
    print(diagnostic)
```

For large files, `stream` yields one transaction at a time, so memory stays bounded:

```python
from pathlib import Path

for batch in oxedi.stream(Path("big.835").read_bytes()):
    services = batch.tables["services"].to_polars()
```

To get a parsed file back byte for byte: `result.document.write()`.

## Write

`oxedi.write` turns tables into an 835. Parse, change a value, write:

```python
import datetime
import polars as pl

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

Pass `spec=result.spec` so the tables are written with the spec that parsed them (4010 or
5010). The writer is strict: if the file would not read back clean, or the money does not
balance, it raises `oxedi.WriteError` listing every finding, and writes nothing. To get the
file anyway, for example to build invalid test files:

```python
try:
    data = oxedi.write(frames, envelope, spec=result.spec)
except oxedi.WriteError as error:
    for finding in error.findings:
        print(finding.table, finding.row, finding.column, finding)

data, findings = oxedi.write(frames, envelope, spec=result.spec, allow_findings=True)
```

## DuckDB

```sql
FROM read_835('remits/*.835', table_name := 'services', filename := true);

-- the findings of the parse, one row each
FROM read_835('remits/*.835', table_name := 'diagnostics', ignore_errors := true);
```

Write tables back with `COPY`. `payments`, `claims` and the rest are tables or views of
those names, for example `CREATE TEMP TABLE claims AS FROM read_835('remittance.835',
table_name := 'claims')` after a change in SQL:

```sql
COPY (
  SELECT {
    'payments':             (SELECT list(t ORDER BY t."row") FROM payments t),
    'claims':               (SELECT list(t ORDER BY t."row") FROM claims t),
    'services':             (SELECT list(t ORDER BY t."row") FROM services t),
    'adjustments':          (SELECT list(t ORDER BY t."row") FROM adjustments t),
    'provider_adjustments': (SELECT list(t ORDER BY t."row") FROM provider_adjustments t)
  }
) TO 'out.835' (FORMAT edi835, sender_id 'ACMEPAYER', receiver_id 'SUNRISECLINIC',
                date DATE '2024-01-10', time TIME '09:00');
```

## Validate with pyx12

```python
from oxedi.pyx12 import validate

findings = result.diagnostics + validate(data)   # bytes, a path or a binary file object
for d in sorted(findings, key=lambda d: d.level):
    print(d.origin, d.code, d)                   # "oxedi" or "pyx12"
```

`parse` never calls pyx12; `validate` does, on demand, and returns the same `Diagnostic` type.

## Spec versions

`parse`, `parse_file` and `stream` read the version a file declares and use the matching
built-in spec: 5010 (`005010X221A1`) by default, 4010 (`004010X091A1`) for 4010 files. Pass
`spec=oxedi.Spec.builtin("4010")` to choose one. A JSON patch adds segments or table columns
without touching the code; see [The spec](docs/spec.md).

## More detail

| Guide | What is in it |
|---|---|
| [Reading](docs/reading.md) | Tables in depth, large files, diagnostics, the document |
| [Writing](docs/writing.md) | The envelope, findings, balancing rules, writing from your own tables |
| [DuckDB](docs/duckdb.md) | `read_835` parameters, `COPY` options, accepted types, limits |
| [The spec](docs/spec.md) | Spec versions and patches |
| [Validating with pyx12](docs/pyx12.md) | What `validate` reports and where |
| [Migrating from edi-835-parser](docs/migrating-from-edi-835-parser.md) | Change the import, or move to the native API |

## Versioning

The project follows semantic versioning. While the version is `0.x`, a minor release may
change the API. See [`CHANGELOG.md`](CHANGELOG.md). The DuckDB extension is versioned
separately.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## License

MIT. See [`LICENSE`](https://github.com/javillegasna/oxedi/blob/master/LICENSE) and [`THIRD_PARTY_NOTICES`](https://github.com/javillegasna/oxedi/blob/master/THIRD_PARTY_NOTICES).
