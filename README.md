# oxedi

A fast X12 EDI parser for Python, written in Rust. Today it reads the 835 (electronic
remittance advice).

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

## DuckDB extension

The same parser is available as a DuckDB extension, `oxedi`, so any DuckDB client can read
835 files in SQL (not yet published to the community repository; see
[`crates/oxedi_duckdb`](crates/oxedi_duckdb/README.md) for building it).

```sql
INSTALL oxedi FROM community;
LOAD oxedi;

FROM read_835('remittance.835');                                   -- claims (the default table)
FROM read_835('remits/*.835', table_name := 'services', filename := true);
FROM read_835(['a.835', 'b.835'], table_name := 'payments');

-- the findings of the parse, one row each
FROM read_835('remits/*.835', table_name := 'diagnostics', ignore_errors := true);

COPY (SELECT * FROM read_835('remits/*.835', table_name := 'claims')) TO 'claims.parquet';
```

`read_835(path, ...)` takes a path, a glob pattern or a list of them, and returns one table
per call. Parameters:

| Parameter | Default | Meaning |
|---|---|---|
| `table_name` | `'claims'` | `payments`, `claims`, `services`, `adjustments`, `provider_adjustments` or `diagnostics` |
| `filename` | `false` | add a `filename` column with the path each row came from |
| `version` | the file's own | force the built-in spec, `'5010'` or `'4010'`; by default each file uses the version it declares (5010 if none) |
| `binary` | `false` | return text columns as `BLOB` (the raw bytes) instead of `VARCHAR` |
| `ignore_errors` | `false` | report a file that is not an X12 interchange as a `diagnostics` row and go on |

Text columns are `VARCHAR`, so a field that is not valid UTF-8 fails the query with an error
that names the file and the cell. Pass `binary := true` to read those fields byte for byte
(nothing is lost to an encoding guess). The other columns are typed as in the Python
tables: decimals, dates and times.

Without `ignore_errors`, a file that is not an interchange fails the query. With it, the
file contributes no rows to any table but one `diagnostics` row whose `rule` is the error,
so run the query with `table_name := 'diagnostics'` as well to see what was skipped.
In that row `datum` holds the bytes found instead of `ISA`: as `escape_ascii` text (always valid
UTF-8) by default, and raw with `binary := true`.

Glob patterns are expanded by a private in-memory DuckDB that honours your settings.
`enable_external_access` and `disabled_filesystems` apply: with `enable_external_access`
off, a glob pattern is an error. A remote pattern such as
`s3://bucket/*.835` sees persistent secrets only, not temporary `CREATE SECRET` ones. Plain
paths and lists are read through your own file system and secrets.

Versions: the extension is versioned separately from the Python package, and its releases are
tagged `duckdb-v*`.

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

Coming from edi-835-parser? See the [migration guide](docs/migrating-from-edi-835-parser.md).

## Versioning

The project follows semantic versioning. While the version is `0.x`, a minor release may change the API. See [`CHANGELOG.md`](CHANGELOG.md).

## Contributing

Prerequisites: the Rust toolchain pinned in `rust-toolchain.toml` (installed by
`rustup`), Python 3.11 or later, and [uv](https://docs.astral.sh/uv/). `make venv`
creates `.venv` with maturin and the test tools.

```bash
make help       # list every target
make venv       # create .venv with the dev tools
make py-dev     # build the extension into .venv (debug)
make gates      # format, clippy, Rust tests, benches compile, docs
make py-test    # build and run the Python test suite
make dist       # build the sdist and the release wheel into target/wheels
make smoke      # install the built wheel in a clean venv and run the suite
```

The files under `crates/oxedi_core/tests/fixtures/` and `crates/oxedi_core/tests/samples/`
are never edited.

## License

MIT. See [`LICENSE`](https://github.com/javillegasna/oxedi/blob/master/LICENSE) and [`THIRD_PARTY_NOTICES`](https://github.com/javillegasna/oxedi/blob/master/THIRD_PARTY_NOTICES).
