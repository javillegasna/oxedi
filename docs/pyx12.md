# Validating with pyx12

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

## What it returns

A file with no `ISA` raises `oxedi.ParseError`, as `parse` does. Findings come in segment order.

`validate` returns `oxedi.Diagnostic`, the type `parse` returns, so the two lists mix, sort by
`level` and filter by `origin`. It reports every error pyx12's engine records, read from the
error tree pyx12 builds: interchange, group and transaction errors, and segment and element
errors, those of the envelope segments included.

A pyx12 finding has:

- `kind == "External"`.
- `origin == "pyx12"`.
- `code` set to pyx12's error code.
- `path` naming the loops open at its segment, as `parse` names them.

Interchange, group and transaction findings are level 1, segment and element findings level 2.

## Where a finding points

A count or control number that does not match lands on the trailer segment, at the element
holding it, with that value as `datum`. A duplicate or missing control structure lands on the header, at the control number when
that is the offending value. When pyx12 gives no value, `datum` is the element's bytes, or the segment id for a
finding with no element.

## Files pyx12 cannot read

A file pyx12 cannot read, or an exception inside pyx12, comes back as one level 1 finding with
no `code` whose `rule` starts with `could not finish validating`, instead of a traceback. A
file pyx12 rejects points at its `ISA` segment, with the `ISA` bytes as `datum`; a failure
after the last segment names the last segment pyx12 completed, and a failure while reading names the
segment pyx12 was processing.

## Logging

pyx12 also logs what it finds under the `pyx12` logger. On first use, `validate` attaches a
`logging.NullHandler` to that logger when it has no handler, so the records are not printed by
default and reach any handler you configure. This also applies to code that runs pyx12
directly in the same process. No level, propagation or `logging.disable` setting is changed.
