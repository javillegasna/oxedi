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

## Writing

`oxedi.write` turns tables back into an 835: the tables of a parse, or your own Arrow, Polars or
pandas tables with the spec's columns. Parse, change a value, write:

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
data = oxedi.write(frames, envelope)  # bytes, one interchange
```

The spec is the one that parsed the tables, else the built-in 5010 spec. A dictionary of frames,
as above, does not carry the parse's spec, so pass the one that matches the file's version: for
a 4010 file, `oxedi.write(frames, envelope, spec=oxedi.Spec.builtin("4010"))`; otherwise it is
written as 5010 and refused. A table left out has no rows and a column left out is null. Rows nest by their `payment`, `claim` and `service` columns, the row
number of their parent. Only what the tables hold is written: segments no column reads (for
example the payer's `PER*CX` or the bank details of `BPR`) are left out. To reproduce a parsed
file byte for byte, use `result.document.write()` instead.

**Envelope.** `oxedi.Envelope` gives what the tables cannot: `sender_id` and `receiver_id` with
their qualifiers (`sender_qualifier`, `receiver_qualifier`, default `"ZZ"`), `date` and `time`,
`usage_indicator`, the first `control_number` (default 1), optional `application_sender` and
`application_receiver` for the group header, `delimiters` (default `*`, `:`, `~` and repetition
`^`) and `line_break` (a line break after each segment). The writer works out the rest: the
fixed-width `ISA`, the control numbers that must match (`ISA13`/`IEA02`, `GS06`/`GE02`,
`ST02`/`SE02`) and the counts (`SE01`, `GE01`, `IEA01`).

**Strict by default.** The written file is read back with the spec, and every diagnostic of that
read is a finding, as is anything the writer cannot place: for example a required element or
occurrence without a value, a value outside its code list or length, a row whose parent does not
exist or that comes out of order, a value holding a delimiter, or money that does not balance.
A value holding a delimiter is reported first: the file then splits where the data does not, so
it is not read back, and the other findings show once that value is fixed. Any finding raises
`oxedi.WriteError`, a `ValueError` whose message lists every finding and whose `findings` holds
them as `oxedi.WriteFinding` (each names the table, row and column, or the envelope field, with
the diagnostic behind it); nothing is written. With `allow_findings=True` the call returns
`(data, findings)` instead, for example to produce invalid files for tests:

```python
try:
    data = oxedi.write(frames, envelope)
except oxedi.WriteError as error:
    for finding in error.findings:
        print(finding.table, finding.row, finding.column, finding)

data, findings = oxedi.write(frames, envelope, allow_findings=True)
```

The writer never changes money: the balancing rules (`BPR02` against the claims' payments and
the `PLB` adjustments, each claim's and each service's charge minus payment against their `CAS`
adjustments) are checked, never used to fill a total. The claim rule does not model interest
(`AMT*I`): a claim whose payment includes interest does not balance and is refused unless
`allow_findings` is set.

**Null and empty text.** In text columns, `null` is an element the segment does not have and `""`
one it has but leaves empty. A null cell in the middle of a segment is still written as an empty
element, so it reads back as `""`; only trailing nulls read back as `null`. The tables of a
parse write back to the same tables.

**From DuckDB.** The `oxedi` extension writes the same file from SQL: `COPY (SELECT {'payments':
(SELECT list(t ORDER BY t."row") FROM payments t), ...}) TO 'out.835' (FORMAT edi835, sender_id
..., receiver_id ..., date ..., time ...)`, byte for byte equal to `oxedi.write` with the same
envelope. See [`crates/oxedi_duckdb`](crates/oxedi_duckdb/README.md#writing).

### From your own tables

`oxedi.write` takes the spec's table and column names and nothing else. It tolerates a missing
table (no rows), a missing column (null) and a column of a wider type (an integer for a big
integer, a decimal of another scale that holds the value). It refuses a table or column that is
not in the spec, floats for money, a value that would change in the conversion, and a file whose
totals do not balance or that lacks a required element, each finding naming the table, row and
column.

The spec's tables key their rows by position: `row` counts the rows of a table from 0, and
`payment`, `claim` and `service` hold the `row` of the parent. If your tables have their own
keys, number the rows with `with_row_index("row")`, bring the parents' `row` in with a join
and drop your helper columns. Rows of one parent must be together and in their parents'
order, and the adjustments of a claim (no `service`) come before those of its services:

```python
import polars as pl

# my_payments (pay_key), my_claims (claim_key, pay_key), my_services (service_key,
# claim_key), my_adjustments (claim_key, service_key or null), my_provider_adjustments
# (pay_key): your tables, each with the spec's other columns
payments = my_payments.sort("pay_key").with_row_index("row")
claims = (
    my_claims.sort("claim_key")
    .join(payments.select("row", "pay_key"), on="pay_key")
    .rename({"row": "payment"})
    .with_row_index("row")
)
claim_rows = claims.select(claim="row", payment="payment", claim_key="claim_key")
services = (
    my_services.sort("service_key").join(claim_rows, on="claim_key").with_row_index("row")
)
adjustments = (
    my_adjustments.join(claim_rows, on="claim_key")
    .join(services.select(service="row", service_key="service_key"), on="service_key", how="left")
    .sort("claim", "service", nulls_last=False, maintain_order=True)
    .with_row_index("row")
)
provider_adjustments = (
    my_provider_adjustments.join(payments.select(payment="row", pay_key="pay_key"), on="pay_key")
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

To see a table's columns today, use `result.tables["claims"].to_polars().schema` (or `.columns`) on any parsed file, or in
the extension `DESCRIBE SELECT * FROM read_835('remittance.835', table_name := 'claims')`; a
file-free schema is tracked in [#138](https://github.com/javillegasna/oxedi/issues/138). The
extension's README has the same mapping in SQL with `row_number()` and joins.

## DuckDB extension

The same parser is available as a DuckDB extension, `oxedi`, so any DuckDB client can read
835 files in SQL. It was accepted into DuckDB's community repository
(duckdb/community-extensions#2937), but its community build failed on Windows on an
OS-specific test text; the fix (extension 0.1.1, PR #120) is pending, and until it lands
`INSTALL oxedi FROM community` is not available. See
[`crates/oxedi_duckdb`](crates/oxedi_duckdb/README.md) for building it.

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

Objects merge key by key, while arrays are replaced whole. A loop's segments are named
occurrences, so a patch adds one (or changes or removes it with `null`) by its name:

```json
{"loops": {"1000A": {"occurrences": {"xx": {"segment": "XX", "pos": 11400}}}}}
```

`pos` orders the occurrences of a loop. The transaction and every loop below it share one
position space: the built-in spec numbers a segment of the transaction's n-th table at
n × 10000 plus its implementation-guide position (1000A's N1 is 10800, 2100's CLP 20100), and
a child loop's occurrences sit at their own positions inside that space. The occurrence a
loop opens on comes first: every other occurrence of the loop has a higher `pos`.

A column reads an element of a segment chosen by `segment` and optional `where`
conditions, or of a named occurrence (`{"occurrence": "patient_name", "element": 3}`). `loop`
reads a loop inside the table's anchor or above it: in the services table,
`{"loop": "2100", "occurrence": "claim_payment_information", "element": 1}` gives each service
its claim's id. When an occurrence repeats, `pick` chooses `"first"` (the default), `"last"` or
the n-th match, counting from 1, among the segments read while the row's loop instance is
open; a loop above the anchor offers the segments its enclosing instance read before the row's
instance opened.

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
