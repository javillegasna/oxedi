# oxedi835

Lossless, fast, data-driven EDI 835 parser core, written in Rust 🦀.

`oxedi835` = oxidación + edi835. Greenfield reboot of the `fast_edi835` POC.

See [`.doc/architectural-commitment.md`](.doc/architectural-commitment.md) for the north star and roadmap.

## Status

**Stage 5 — Python binding.** `pip install` from source builds one `abi3` wheel for
Python 3.11 and later. `oxedi835.parse` reads a whole file with the GIL released and
returns the lossless document, the typed tables and every diagnostic as a value;
`oxedi835.stream` yields the tables one transaction (or any loop) at a time with memory
bounded by that loop. Tables reach Polars, pyarrow or DuckDB through the Arrow PyCapsule
interface without copying.

## Python

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
