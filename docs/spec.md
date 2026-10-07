# The spec

The structure of the 835 and the columns of each table are defined by a JSON spec. You can
choose a version, or adapt the spec to a payer's variations with a patch, without touching
the code. `parse`, `parse_file` and `stream` all accept `spec=`.

## Versions

`parse`, `parse_file` and `stream` read the version a file declares (`ISA12` and `GS08`) and
use the matching built-in spec: 5010 (`005010X221A1`) by default, 4010 (`004010X091A1`) for
4010 files. Pass `spec=` to override, or pick one yourself:

```python
spec = oxedi.Spec.builtin(version="4010")
result = oxedi.parse(data, spec=spec)
```

`result.spec` is the spec that projected the tables: the one you gave, or the built-in one
selected from the file's version.

## Patching the spec

A patch in JSON Merge Patch format (RFC 7386) adapts the spec to a payer's variations. This
one adds a column:

```python
spec = oxedi.Spec.builtin().patch({
    "tables": {"claims": {"columns": {
        "contract_class": {"segment": "REF", "where": {"1": "CE"}, "element": 2}
    }}}
})
result = oxedi.parse(data, spec=spec)   # claims now has a contract_class column
```

Objects merge key by key, while arrays are replaced whole.

## Segments and positions

A loop's segments are named occurrences, so a patch adds one (or changes or removes it with
`null`) by its name:

```json
{"loops": {"1000A": {"occurrences": {"xx": {"segment": "XX", "pos": 11400}}}}}
```

`pos` orders the occurrences of a loop. The transaction and every loop below it share one
position space: the built-in spec numbers a segment of the transaction's n-th table at
n × 10000 plus its implementation-guide position (1000A's N1 is 10800, 2100's CLP 20100), and a
child loop's occurrences sit at their own positions inside that space. The occurrence a loop
opens on comes first: every other occurrence of the loop has a higher `pos`.

## Columns

A column reads an element of a segment chosen by `segment` and optional `where` conditions, or
of a named occurrence (`{"occurrence": "patient_name", "element": 3}`).

`loop` reads a loop inside the table's anchor or above it: in the services table,
`{"loop": "2100", "occurrence": "claim_payment_information", "element": 1}` gives each service
its claim's id.

When an occurrence repeats, `pick` chooses `"first"` (the default), `"last"` or the n-th match,
counting from 1, among the segments read while the row's loop instance is open; a loop above
the anchor offers the segments its enclosing instance read before the row's instance opened.

## Writing with a custom spec

A patched spec also writes: pass it to `oxedi.write(frames, envelope, spec=spec)`. See
[Writing](writing.md).
