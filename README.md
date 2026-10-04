# oxedi835

Lossless, fast, data-driven EDI 835 parser core, written in Rust 🦀.

`oxedi835` = oxidación + edi835. Greenfield reboot of the `fast_edi835` POC.

See [`.doc/architectural-commitment.md`](.doc/architectural-commitment.md) for the north star and roadmap.

## Status

**Stage 5 — Python binding.** Building from source (`maturin develop`) produces one `abi3` wheel for
Python 3.11 and later. `oxedi835.parse` reads a whole file with the GIL released and
returns the lossless document, the typed tables and every diagnostic as a value;
`oxedi835.stream` yields the tables one transaction (or any loop) at a time with memory
bounded by that loop. Tables reach Polars, pyarrow or DuckDB through the Arrow PyCapsule
interface without copying.

## Python

`make help` lists the developer targets (Rust gates, `py-dev`, `py-test`, `dist`, `smoke`, publishing).

From a clone (publishing to PyPI comes later):

```bash
uv venv && source .venv/bin/activate
uv pip install maturin && maturin develop --uv --release --manifest-path crates/oxedi835_py/Cargo.toml
```

```python
import oxedi835, polars as pl
from pathlib import Path

result = oxedi835.parse_file("remittance.835")
claims = pl.DataFrame(result.tables["claims"])                # zero-copy, through Arrow
problems = [str(d) for d in result.diagnostics]               # values, never raised
for batch in oxedi835.stream(Path("big.835").read_bytes()):   # one transaction at a time
    services = pl.DataFrame(batch.tables["services"])
```

`Spec.builtin().patch({...})` extends the structure with the same JSON patches as below;
`parse(data, spec=...)` and `stream(data, spec=..., by="2100")` take the result.

## Coming from edi-835-parser

Two paths. **Change the import** and keep your code:

```python
# before: from edi_835_parser import parse
from oxedi835.edi_835_parser import parse      # pip install "oxedi835[edi-835-parser]"

frame = parse("remittances/").to_dataframe()    # same rows, columns, order and dtypes
# parse_bytes(data, file_path=...), parse_file_obj(f), parse_many([...]) read from memory
extra = parse("remittances/").to_dataframe(extended=True)  # + claims without services, claim
                                                # adjustments, PLB, unmapped REF/AMT (x_ columns)
```

The frame equals edi-835-parser 1.8.0's cell for cell on our test files; payer ids that
are not numbers (`N104` with qualifier `XV`) work. `extended=True` keeps those columns in
the same order and only adds columns that start with `x_` and the rows the library drops;
when claim-only or provider-adjustment rows exist, some strict columns widen their dtype
(`int` to `float`, `bool` to `object`, and an `object` column holding only `None` to
`float64` with NaN). Differences from the library:

- One `TransactionSet` per `ST`, not per file; the separators come from the ISA instead of
  being guessed per element; an unknown claim status gives `"unknown"` instead of an error.
- A second `NM1*QC`, `NM1*82` or `DTM*232`/`233` in a claim: the library raises
  `AssertionError`, we take the first. A claim without `NM1*QC`: the library raises
  `AssertionError`, we give `patient` as `None` and count it once in `count_patients`.
- `parse` also accepts path-like objects (`pathlib.Path`), where the library raises
  `TypeError`; `""` gives `FileNotFoundError` instead of `IndexError`.
- Arbitrary non-whitespace bytes before the ISA (other than a UTF-8 BOM followed by ASCII
  whitespace), a doubled BOM, or non-ASCII whitespace after a BOM give an empty result where
  the library reads the segments after the first.
- A vertical tab or form feed before the ISA without a BOM gives an empty result.
- Without a BOM, an unreadable ISA preceded by a newline gives an empty result where the
  library raises `IndexError`.
- The `ParseError` for a single path names the file.
- The native API (`oxedi835.parse`) is strict and raises `ParseError` for all of these.
  It does read a file that starts with a UTF-8 BOM (then optional spaces, tabs or line
  breaks) before the ISA: the mark stays in the first segment's `raw`, `write()` gives the
  file back unchanged, and a `ByteOrderMark` diagnostic says it was there. The
  compatibility layer keeps the library's reading (no `interchange` for such a file).

**Or move to the native API**, which needs no pandas:

| edi-835-parser | oxedi835 |
|---|---|
| `parse(path)` | `oxedi835.parse_file(path)` returning a `Result` |
| `.to_dataframe()` | `result.tables["services"].to_polars()` / `.to_pandas()` (`pip install "oxedi835[polars]"` or `"oxedi835[pandas]"`), joined to `claims` on `claim` and to `payments` on `payment` |
| `.count_claims()` | `result.count_claims()` |
| `.count_patients()` | `result.count_patients()` (a null id is not a patient) |
| `.sum_payments()` (float) | `result.sum_payments()` (`Decimal`) |
| `transaction_set.payer` / `.payee` | `result.payer` / `result.payee` (dict of text values: `name`, `identification_code`, `address`, `city`, `state`, `zip_code`; `ValueError` when `payments` has more than one row) |
| none | SQL: `duckdb.sql("select ... from claims")` with `claims = result.tables["claims"]` |

## Extending the 835 spec

Patches use JSON Merge Patch (RFC 7386). A patch replaces arrays wholesale: to add a
segment to a loop, list the loop's full `segments`; objects merge key by key, so
changing a `trigger` or `end` does not touch `segments`.

```json
{
  "loops": {
    "1000A": { "segments": ["N3", "N4", "REF", "PER", "XX"] }
  }
}
```

A `tables` patch adds a column the same way. This one reads a payer's `REF*CE`
reference, and the projected `claims` table gains a `contract_class` column:

```json
{"tables":{"claims":{"columns":{
  "contract_class":{"segment":"REF","where":{"1":"CE"},"element":2}
}}}}
```
