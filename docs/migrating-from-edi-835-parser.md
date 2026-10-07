# Migrating from edi-835-parser

This guide is for projects that use the `edi-835-parser` library and want to move to `oxedi`.
There are two paths: keep your code and change the import, or move to the native API.

## Switch the import

```bash
pip install "oxedi[edi-835-parser]"
```

```python
# before: from edi_835_parser import parse
from oxedi.edi_835_parser import parse

frame = parse("remittances/").to_dataframe()    # same rows, columns, order and dtypes
```

`oxedi.edi_835_parser` exposes the same API as the library. `parse` reads a file, or every
`.txt`, `.835` and `.DAT` file of a folder. `parse_bytes(data, file_path=...)`,
`parse_file_obj(f)` and `parse_many([...])` (bytes or binary file objects) read from memory.

The frame equals edi-835-parser 1.8.0's cell for cell on our test files. Payer ids that are not
numbers (`N104` with qualifier `XV`) work.

## What changes for you

These are the differences you are most likely to notice.

- **One `TransactionSet` per `ST`**, not per file. The separators come from the ISA instead of
  being guessed per element.
- **Repeated segments take the first.** A second `NM1*QC`, `NM1*82` or `DTM*232`/`233` in a
  claim makes the library raise `AssertionError`; we take the first.
- **A claim without `NM1*QC`.** The library raises `AssertionError`; we give `patient` as
  `None` and count it once in `count_patients`.
- **An unknown claim status** gives `"unknown"` instead of an error.
- **Path-like inputs.** `parse` also accepts `pathlib.Path`, where the library raises
  `TypeError`. `""` gives `FileNotFoundError` instead of `IndexError`.
- **Errors name the file.** The `ParseError` for a single path names the file.

### `extended=True`

```python
extra = parse("remittances/").to_dataframe(extended=True)
```

This adds what the library drops: claims without services, claim adjustments, PLB and unmapped
REF/AMT. It keeps the strict columns in the same order and only adds columns that start with
`x_`, plus the rows the library drops. When claim-only or provider-adjustment rows exist, some
strict columns widen their dtype: `int` to `float`, `bool` to `object`, and an `object` column
holding only `None` to `float64` with NaN.

## Edge cases

These matter only for unusual files. The compatibility layer gives an empty result for these;
the library reads on or raises:

- Arbitrary non-whitespace bytes before the ISA (other than a UTF-8 BOM followed by ASCII
  whitespace), a doubled BOM, or non-ASCII whitespace after a BOM give an empty result where
  the library reads the segments after the first.
- A vertical tab or form feed before the ISA without a BOM gives an empty result.
- Without a BOM, an unreadable ISA preceded by a newline gives an empty result where the
  library raises `IndexError`.

The native API (`oxedi.parse`) is strict and raises `ParseError` for all of these. It does read
a file that starts with a UTF-8 BOM (then optional spaces, tabs or line breaks) before the ISA:
the mark stays in the first segment's `raw`, `write()` gives the file back unchanged, and a
`ByteOrderMark` diagnostic says it was there. The compatibility layer keeps the library's
reading (no `interchange` for such a file).

## Move to the native API

The native API needs no pandas:

| edi-835-parser | oxedi |
|---|---|
| `parse(path)` | `oxedi.parse_file(path)` returning a `Result` |
| `.to_dataframe()` | `result.tables["services"].to_polars()` / `.to_pandas()` (`pip install "oxedi[polars]"` or `"oxedi[pandas]"`), joined to `claims` on `claim` and to `payments` on `payment` |
| `.count_claims()` | `result.count_claims()` |
| `.count_patients()` | `result.count_patients()` (a null id is not a patient; ids are text, so `0123` and `123` are different patients, unlike edi-835-parser) |
| `.sum_payments()` (float) | `result.sum_payments()` (`Decimal`) |
| `transaction_set.payer` / `.payee` | `result.payer` / `result.payee` (dict of text values: `name`, `identification_code`, `address`, `city`, `state`, `zip_code`; `ValueError` when `payments` has more than one row) |
| none | SQL: `duckdb.sql("select ... from claims")` with `claims = result.tables["claims"]` |

## Extras and pandas

The extras pin pandas differently: `oxedi[pandas]` asks for `pandas>=2` and so allows pandas 3,
while `oxedi[edi-835-parser]` pins `pandas>=2.0.3,<3`, the range the compatibility layer's
parity with edi-835-parser 1.8.0 is tested on.
