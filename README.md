# oxedi835

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
pip install oxedi835               # no Python dependencies
pip install "oxedi835[polars]"     # with Polars
pip install "oxedi835[pandas]"     # with pandas and pyarrow
```

Requires Python 3.11 or later. Wheels are available for Linux (x86_64 and aarch64, plus
musl on x86_64), macOS (Intel and Apple silicon) and Windows (x86_64).

## Quick start

```python
import oxedi835

result = oxedi835.parse_file("remittance.835")

claims = result.tables["claims"].to_polars()      # or .to_pandas()
services = result.tables["services"].to_polars()

print(result.count_claims(), result.sum_payments())  # sum_payments() is a Decimal
print(result.payer["name"], result.payee["name"])

for diagnostic in result.diagnostics:
    print(diagnostic)
```

`oxedi835.parse(data)` does the same from `bytes`. A file that is not an 835 at all (for
example, one that does not start with an `ISA` segment) raises `oxedi835.ParseError`.

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

for batch in oxedi835.stream(Path("big.835").read_bytes()):
    services = batch.tables["services"].to_polars()
```

Each batch holds the tables and diagnostics of one transaction. Pass `by="2100"` (or any
other loop id) to get one batch per claim instead. Parsing releases the GIL, so several
files can be parsed in parallel from threads.

### Writing the file back

```python
data = Path("remittance.835").read_bytes()
assert oxedi835.parse(data).document.write() == data
```

## Extending the spec

The structure of the 835 and the columns of each table are defined by a JSON spec. A patch
in JSON Merge Patch format (RFC 7386) adapts it to a payer's variations:

```python
spec = oxedi835.Spec.builtin().patch({
    "tables": {"claims": {"columns": {
        "contract_class": {"segment": "REF", "where": {"1": "CE"}, "element": 2}
    }}}
})
result = oxedi835.parse(data, spec=spec)   # claims now has a contract_class column
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
spec = oxedi835.Spec.builtin(version="4010")
```

## Validating with pyx12

`pip install "oxedi835[pyx12]"` adds [pyx12](https://github.com/azoner/pyx12)'s
implementation-guide validation. `parse` never calls it; `validate` does, on demand:

```python
from oxedi835.pyx12 import validate

for finding in validate(data):    # bytes, a path or a binary file object
    print(finding)                # origin, rule, code, segment, byte range, element, datum
```

Each finding carries the segment index and byte range in your file. An exception inside pyx12
comes back as one `Pyx12Failure` finding instead of a traceback.

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

The files under `crates/edi835_core/tests/fixtures/` and `crates/edi835_core/tests/samples/`
are never edited.

## License

MIT. See [`LICENSE`](https://github.com/javillegasna/oxedi835/blob/master/LICENSE) and [`THIRD_PARTY_NOTICES`](https://github.com/javillegasna/oxedi835/blob/master/THIRD_PARTY_NOTICES).
