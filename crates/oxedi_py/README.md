# oxedi

A fast, lossless parser and writer for X12 EDI 835 files (electronic remittance advice). The
core is written in Rust and is available from Python and from DuckDB.

- **Lossless.** Every byte of the file is kept. Nothing is silently dropped, and the file can
  be written back exactly as it was read.
- **Ready-made tables.** Payments, claims, service lines and adjustments come out as typed
  tables that open in Polars, pandas, pyarrow or DuckDB.
- **Problems are reported, not raised.** Questionable data still parses. Each issue is a
  diagnostic that names the rule, the position and the value.

Documentation: **<https://javillegas.dev/oxedi/latest/>**. New here? Start with the
[five-minute tutorial](https://javillegas.dev/oxedi/latest/start/), or
**[try it in your browser](https://javillegas.dev/oxedi/viewer/)**: drop a file on the page and
see its tables and problems. Nothing is uploaded.

## Install

```bash
pip install oxedi                    # no Python dependencies
pip install "oxedi[polars]"          # or [pandas], [pyx12], [edi-835-parser]
```

Requires Python 3.11 or later. For DuckDB:

<!-- docs-test: skip the runner loads the locally built extension; INSTALL needs the network -->
```sql
INSTALL oxedi FROM community;
LOAD oxedi;
```

`INSTALL` gives extension 0.1.x, which reads files. Writing with `COPY` needs 0.2.0, which is
not in the community repository yet. See [DuckDB](https://javillegas.dev/oxedi/latest/duckdb/).

## Read

```python
import oxedi

result = oxedi.parse_file("remittance.835")      # or oxedi.parse(data) from bytes

claims = result.tables["claims"].to_polars()     # or .to_pandas()
print(result.summary("count_claims"), result.summary("sum_payments"))  # a sum is a Decimal
print(result.summary("payer")["name"], result.summary("payee")["name"])

for diagnostic in result.diagnostics:            # problems come back as data
    print(diagnostic)
```

A file that is not an 835 raises `oxedi.ParseError`; anything else parses. `oxedi.stream`
yields one transaction at a time for large files.

## Write

Parse, change a value, write:

```python
import datetime
import polars as pl

frames = result.tables.to_polars()               # {table name: polars frame}
frames["payments"] = frames["payments"].with_columns(trace_number=pl.lit("CHK100235"))

envelope = oxedi.Envelope(
    sender_id="ACMEPAYER",
    receiver_id="SUNRISECLINIC",
    date=datetime.date(2024, 1, 10),
    time=datetime.time(9, 0),
)
data = oxedi.write(frames, envelope, spec=result.spec)  # bytes, one interchange
```

The writer is strict: if the money does not balance or the file would not read back clean, it
raises `oxedi.WriteError` and writes nothing.

## DuckDB

```sql
FROM read_835('remits/*.835', table_name := 'services', filename := true);

-- the diagnostics of the parse, one row each
FROM read_835('remits/*.835', table_name := 'diagnostics', ignore_errors := true);
```

## Documentation

| Page | What is in it |
|---|---|
| [Get started](https://javillegas.dev/oxedi/latest/start/) | A five-minute tutorial on an example file |
| [The 835 in five minutes](https://javillegas.dev/oxedi/latest/start/the-835/) | What a remittance holds and how it maps to the tables |
| [Reading](https://javillegas.dev/oxedi/latest/python/reading/) | Tables in depth, [large files](https://javillegas.dev/oxedi/latest/python/large-files/), [diagnostics](https://javillegas.dev/oxedi/latest/python/diagnostics/) |
| [Writing](https://javillegas.dev/oxedi/latest/python/writing/) | The envelope, [your own tables](https://javillegas.dev/oxedi/latest/python/own-tables/), [when the writer refuses](https://javillegas.dev/oxedi/latest/python/when-write-refuses/) |
| [DuckDB](https://javillegas.dev/oxedi/latest/duckdb/) | `read_835` parameters, `COPY` options, limits |
| [The spec](https://javillegas.dev/oxedi/latest/customize/spec/) | Spec versions (5010, 4010), patches, and [packages](https://javillegas.dev/oxedi/latest/customize/packages/) with how to check them |
| [Validating with pyx12](https://javillegas.dev/oxedi/latest/python/pyx12/) | What `validate` reports and where |
| [Recipes](https://javillegas.dev/oxedi/latest/recipes/lakehouse/) | A Parquet lakehouse, Polars analytics, pipelines |
| [Tables and columns](https://javillegas.dev/oxedi/latest/reference/tables/) | Every column, its type and its source element; a [glossary](https://javillegas.dev/oxedi/latest/reference/glossary/) |
| [API reference](https://javillegas.dev/oxedi/latest/reference/api/parsing/) | Every public function and class |
| [Migrating from edi-835-parser](https://javillegas.dev/oxedi/latest/migrating-from-edi-835-parser/) | Change the import, or move to the native API |
| [Roadmap](https://javillegas.dev/oxedi/latest/roadmap/) | What is available, what comes next, what is being considered |

## Versioning, contributing, license

Semantic versioning; while the version is `0.x`, a minor release may change the API. See
[`CHANGELOG.md`](https://github.com/javillegasna/oxedi/blob/master/CHANGELOG.md), [`CONTRIBUTING.md`](https://github.com/javillegasna/oxedi/blob/master/CONTRIBUTING.md) and
[`ARCHITECTURE.md`](https://github.com/javillegasna/oxedi/blob/master/ARCHITECTURE.md). MIT license, see [`LICENSE`](https://github.com/javillegasna/oxedi/blob/master/LICENSE) and [`THIRD_PARTY_NOTICES`](https://github.com/javillegasna/oxedi/blob/master/THIRD_PARTY_NOTICES).
