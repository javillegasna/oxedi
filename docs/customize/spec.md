# Customizing the spec

A JSON spec defines the structure of the 835 and the columns of each table. You can choose a
version, or adapt the spec to a payer's variations with a patch, without touching the code.
`parse`, `parse_file` and `stream` all accept `spec=`.

The built-in specs are stacks of packages, one per layer. To write a package of your own, check it
and run its conformance kit, see [Packages and layers](packages.md).

## Choose a version

By default oxedi reads the version a file declares (`ISA12` and `GS08`) and uses the matching
built-in spec:

- 5010 (`005010X221A1`), the default.
- 4010 (`004010X091A1`), for 4010 files.

Pass `spec=` to override it, or pick one yourself:

```python
from pathlib import Path
import oxedi

data = Path("remittance-4010.835").read_bytes()
spec = oxedi.Spec.builtin(version=oxedi.Version.V4010)
result = oxedi.parse(data, spec=spec)
print(result.spec, result.count_claims())
```

```text
Spec(name='835', loops=8, tables=5) 4
```

`result.spec` is the spec that projected the tables: the one you gave, or the built-in one
selected from the file's version.

## Add a column with a patch

A patch in JSON Merge Patch format (RFC 7386) adapts the spec. This one adds a column to
`claims`, read from `REF02` of the claim's `REF*CE`:

```python
spec = oxedi.Spec.builtin().patch({
    "tables": {"claims": {"columns": {
        "contract_class": {"segment": "REF", "where": {"1": "CE"}, "element": 2}
    }}}
})
result = oxedi.parse(Path("remittance.835").read_bytes(), spec=spec)
print("contract_class" in result.tables["claims"].columns)
```

```text
True
```

- Objects merge key by key.
- Arrays are replaced whole.
- `patch` accepts JSON text or a mapping of JSON values (`oxedi.JsonObject`). It raises
  `oxedi.SpecError` when the result is not a valid spec, and `TypeError`, naming the place, for a
  value JSON cannot hold (a set, bytes, NaN, a key that is not `str`).

## Choose where a column reads

A column reads one element of one segment. You pick the segment in one of two ways:

- `segment`, with optional `where` conditions on its elements:
  `{"segment": "REF", "where": {"1": "CE"}, "element": 2}`.
- A named occurrence of the loop: `{"occurrence": "patient_name", "element": 3}`.

Add `loop` to read a loop inside the table's anchor or above it. In the services table, this
column gives each service its claim's id:

```python
import polars as pl

spec = oxedi.Spec.builtin().patch({
    "tables": {"services": {"columns": {
        "claim_id": {"loop": "2100", "occurrence": "claim_payment_information", "element": 1}
    }}}
})
services = oxedi.parse_file("remittance.835", spec=spec).tables["services"].to_polars()
print(services.select("row", pl.col("claim_id", "procedure_code").cast(pl.String)))
```

```text
┌─────┬──────────┬────────────────┐
│ row ┆ claim_id ┆ procedure_code │
│ --- ┆ ---      ┆ ---            │
│ i64 ┆ str      ┆ str            │
╞═════╪══════════╪════════════════╡
│ 0   ┆ CLM1001  ┆ 99213          │
│ 1   ┆ CLM1001  ┆ 85025          │
│ 2   ┆ CLM1002  ┆ 97110          │
│ 3   ┆ CLM2001  ┆ 99212          │
└─────┴──────────┴────────────────┘
```

`Spec.column_source(table, column)` says which segment, element and component a column reads, as the spec resolved it:

```python
print(spec.column_source("services", "claim_id"))
```

```text
ElementSource(segment=b'CLP', element=1, component=None)
```

??? note "Details: when the segment repeats"
    When an occurrence repeats, `pick` chooses `"first"` (the default), `"last"` or the n-th
    match, counting from 1. It chooses among the segments read while the row's loop instance
    is open. A loop above the anchor offers the segments its enclosing instance read before the
    row's instance opened.

??? note "Details: which segments a segment column reads"
    When the spec loads, oxedi works out the one occurrence of the reading loop that a column's
    `segment` and `where` choose: the occurrence whose qualifier the `where` fixes, or the
    segment's only occurrence when it has no qualifier. The column reads only the segments the
    parser placed in that occurrence that hold the `where` conditions, and the writer writes the
    value there. When no one occurrence fits (the `where` does not fix the qualifier, or fixes it
    to a code no occurrence lists), the column reads every segment of that id that holds the
    conditions. The writer writes such a column only when the `where` leaves the qualifier free
    and the loop has one occurrence of the segment; otherwise it refuses the spec, naming the
    occurrences it could be (none when no occurrence takes the code).

## Flag patient data

Which columns [hide patient data](../python/without-patient-data.md) when you pass a mask is
part of the spec. A column carries a `phi` key with one of three words:

- `"name"` and `"id"` become a keyed token. They need a text column (`AN` or `ID` element, or
  one the spec does not type).
- `"date"` shifts or reduces a date. It needs a date (`DT`) column.

A table can also carry `phi_seed`, the name of one `id` column. With `dates="shift"` its value
decides how many days the row and the rows under it move; the built-in `claims` table uses
`patient_id`.

```python
spec = oxedi.Spec.builtin().patch({
    "tables": {"claims": {"columns": {
        "contract_class": {"segment": "REF", "where": {"1": "CE"}, "element": 2, "phi": "id"}
    }}}
})
mask = oxedi.Mask("a-secret-of-at-least-16-bytes")
result = oxedi.parse(Path("remittance.835").read_bytes(), spec=spec, mask=mask)
print("contract_class" in spec.schema()["claims"].names)
```

```text
True
```

A patch that flags a numeric column, uses another word, flags a column that is not a date with
`"date"`, names a `phi_seed` column the table lacks, or names one that is not flagged `"id"`
raises `oxedi.SpecError` with the table, the column and the reason. A patch that removes or
unflags `claims.patient_id` must also set `"phi_seed": null` on `claims` (or name another `id`
column).

## Add or change a segment

A loop's segments are named occurrences. A patch adds one by its name, or changes it, or
removes it with `null`:

```json
{"loops": {"1000A": {"occurrences": {"xx": {"segment": "XX", "pos": 11400}}}}}
```

`pos` orders the occurrences of a loop. The occurrence a loop opens on comes first: every other
occurrence of the loop has a higher `pos`.

??? note "Details: how positions are numbered"
    The transaction and every loop below it share one position space. The built-in spec
    numbers a segment of the transaction's n-th table at n × 10000 plus its
    implementation-guide position: 1000A's N1 is 10800, 2100's CLP 20100. A child loop's
    occurrences sit at their own positions inside that space.

## Write with a custom spec

A patched spec also writes: pass it to `oxedi.write(frames, envelope, spec=spec)`. See
[Writing](../python/writing.md). Each column is written where it is read.

## Spec keys reference

Every key a spec can hold, with what it means and the Rust code that reads it. In a package, the
layer that may write each key is in [Who writes which key](packages.md#who-writes-which-key). A spec has two
layers in the core. Loading, in `oxedi_core::spec_load`, reads the whole spec, its top level
included, once as its `SpecModel`, whose parts are the typed `*Model`s (`LoopModel`,
`TableModel`, `SegmentModel`, `SummaryModel`, `BalanceModel`), each declared in
`crates/oxedi_core/src/spec_load/model/`, then checks the model and compiles it into the schema:
the definitions of `oxedi_core::spec`, which reading and writing use. In the Rust column, files
are under `crates/oxedi_core/src/`: a `spec_load/` file compiles and checks the key, a `spec/`
file holds its compiled definition, and any other path is code that uses it. For the internal
names, see the [glossary](https://github.com/javillegasna/oxedi/blob/master/ARCHITECTURE.md#glossary).
A load error names the failing key by its path from the root: an unknown key is an error that
names its path and the keys allowed there, a missing required key one that names its path only,
and a value of the wrong type one that names its path and what it expected, as in
`spec: at version.element: invalid type: string "8", expected usize`. Positions of elements and
components are 1-based. The code names differ from the JSON in three places: a table's
`loops` are its *anchors*, `end` is a loop's *closer*, and `ref` is the *reference* name.

### Root

| Key | Required | Meaning | Rust |
|-----|----------|---------|------|
| `name` | yes | The spec's name, shown in `Spec(name=...)`. A package does not write it: a stack takes it from its top package's id. | `Spec::name`, `spec_load/build.rs` |
| `loops` | yes | The loop structure, by loop name; at least one loop. | `LoopDef`, `spec_load/build.rs`, `spec_load/build_loops.rs` |
| `segments` | no | Names and types the elements of each segment id. | `SegmentDef`, `spec_load/segments.rs` |
| `tables` | no | The tables of the projection, by table name. | `TableDef`, `spec_load/compile.rs` |
| `balancing` | no | Amounts that must add up inside a loop, by rule name. | `BalanceRule`, `spec_load/balance.rs` |
| `version` | no | Which versions of the format the spec covers, used to choose a built-in spec. | `DeclaredVersion`, `spec/version.rs` |
| `summaries` | no | Figures computed over the tables, by summary name. | `Summary`, `spec_load/summaries.rs` |
| `default_table` | no | The table a reader shows when the caller names none. | `Spec::default_table`, `spec_load/summaries.rs` |

### `version`

| Key | Meaning | Rust |
|-----|---------|------|
| `segment` | Id of the segment that carries the version, for example `GS`. | `DeclaredVersion::segment` |
| `element` | Position of the element that holds it, for example `8`. | `DeclaredVersion::element` |
| `values` | The values of that element the spec covers. When left out, the codes of that element in `segments`, in the order written. | `DeclaredVersion::values` |

`segment` and `element` are required. The first segment with that id decides.

### `loops.<name>`

| Key | Required | Meaning | Rust |
|-----|----------|---------|------|
| `parent` | no | Name of the enclosing loop; none for a top-level loop. | `LoopDef::parent` |
| `trigger` | yes | The segment that opens the loop (see below). | `Trigger`, `spec/loops.rs` |
| `occurrences` | no | The named places segments take in the loop (see below). | `OccurrenceDef`, `spec/occurrences.rs` |
| `usage` | no | `required` or `situational` (the default): whether every instance of the parent holds an instance of this loop. | `Usage`, read by `check/occurrences.rs` |
| `max` | no | Most instances under one parent instance; at least 1. | `LoopDef::max`, `check/occurrences.rs` |
| `end` | no | The segment that closes the loop; the loop captures it. It cannot be the trigger's segment. | `LoopDef::end`, `engine/mod.rs` |
| `control` | no | What the `end` segment must agree on with the trigger; needs `end`. | `Control`, `spec/loops.rs`; checked by `check/mod.rs` |

`trigger`:

| Key | Required | Meaning |
|-----|----------|---------|
| `segment` | yes | Segment id. |
| `where` | no | Conditions on the segment's elements: `{"<position>": "<value>"}`. All must hold. |

`control`, four required keys and one optional (this is how `ST`/`SE`, `GS`/`GE` and `ISA`/`IEA` are checked):

| Key | Meaning |
|-----|---------|
| `opener_element` | Position of the control number in the trigger, for example 2 for `ST02`. |
| `closer_element` | Position of the same number in the `end` segment, for example 2 for `SE02`. |
| `count_element` | Position of the count in the `end` segment, for example 1 for `SE01`. |
| `count` | What the count counts: `segments` (trigger to end, both included) or `children` (child loop instances). |
| `fields` | Optional. The caller's field each element of the trigger carries, `{"<position>": "<field>"}`: `sender_qualifier`, `sender_id`, `receiver_qualifier`, `receiver_id`, `application_sender`, `application_receiver`, `date`, `time` or `usage_indicator`. Not on `opener_element`, one element per field, on an element the trigger segment defines, and not on ISA11, ISA12 or ISA16 of the top loop, which the syntax layer owns (the repetition separator, the version and the component separator). |

### `loops.<name>.occurrences.<name>`

An occurrence is a named place for a segment inside a loop. The occurrence the loop's trigger
opens on has the lowest `pos` of its loop.

| Key | Required | Meaning | Rust |
|-----|----------|---------|------|
| `segment` | yes | Segment id. | `OccurrenceDef::segment` |
| `pos` | yes | Order inside the loop; see "how positions are numbered" above. | `OccurrenceDef::pos` |
| `usage` | no | `required` or `situational` (the default). | `OccurrenceDef::usage` |
| `max` | no | Most times it may repeat in one instance. | `OccurrenceDef::max` |
| `qualifier` | no | Tells this occurrence apart from others of the same segment in the loop. | `Qualifier` |
| `elements` | no | How the occurrence narrows its segment's elements, by `"<element>"` or `"<element>-<component>"`: an entry with any of `usage` (`required` or `situational`), `min`, `max` and `codes`, each replacing the element's for a segment at this occurrence. An entry only narrows: `situational` is refused on an element that is required, `min` and `max` stay inside the element's lengths, and `codes` is a subset of the element's `codes` when the element has any (on an element with none, it may hold any codes of the element's type) and fits the lengths that apply. `codes`, `min` and `max` are refused at the qualifier's own place. | `OccurrenceDef::elements` |

`qualifier`: `element` (required), `component` (optional) and `codes` (required, the values
that identify the occurrence). Two occurrences of one segment in a loop need qualifiers on the
same place with no code in common. Occurrences are checked by `check/occurrences.rs`; columns and
balancing rules refer to them by name.

### `segments.<id>.elements.<position>`

The only key of a segment is `elements`. An element or a component takes these keys:

| Key | Required | Meaning | Rust |
|-----|----------|---------|------|
| `name` | yes | Name used downstream; unique among its siblings. | `ElementDef::name` |
| `type` | yes | `AN`, `ID`, `N0` to `N9`, `R`, `DT` or `TM`. | `ElementType`, `spec_load/segments.rs` |
| `usage` | no | `required`: the element must be present and not empty; `situational` (the default) lets it be absent or empty. | `ElementDef::required` |
| `min`, `max` | no | Length bounds, counted as X12 counts them (digits only for `N` and `R`). | `ElementDef` |
| `scale` | no | Decimal places of an `R` element, 2 by default, at most 18. Only `R` accepts it. | `ElementType::R` |
| `codes` | no | The values the element may hold: non-empty, no empty or repeated code, each within `min` and `max`. Refused on `N0` to `N9` and `R`, and on an element with `composite`. | `ElementDef::codes` |
| `composite` | no | Components by position, with these same keys. Only on `AN`, and not nested. | `ElementDef::composite` |

The element checks of `project` and the writer's formatting read these.

### `tables.<name>`

| Key | Required | Meaning | Rust |
|-----|----------|---------|------|
| `loops` | yes | The anchors: one row per instance of each listed loop. In a table without `segment`, no anchor may sit inside another, and a loop anchors only one such table. | `TableDef::loops` |
| `ref` | no | The name of the column that tables below this one use to refer to a row of it. Defaults to the table's name; unique among tables. | `TableDef::reference` |
| `segment` | no | One row per occurrence of this segment in an anchor, instead of per loop instance. | `TableDef::segment` |
| `repeat` | no | One row per element group of `segment`; needs `segment`. | `Repeat` |
| `columns` | no | The declared columns, by name. | `ColumnSource`, `spec_load/columns.rs` |
| `phi_seed` | no | Name of one column flagged `"id"`; its value decides how far the row's dates shift. | `TableDef::phi_seed`, `spec_load/phi.rs` |

`repeat`, both required: `from` (position of the first group's first element) and `step`
(elements per group). Both are at least 1.

The names `row` and `segment` are reserved for columns and for `ref`. The rows of a table are
built by `oxedi_core::project`.

### `tables.<name>.columns.<name>`

A column has exactly one source: `element`, `group_element` or `segment_index`.

| Key | Meaning |
|-----|---------|
| `element` | Position of the element to read. |
| `component` | Position of the component of a composite element (with `element` or `group_element`). |
| `group_element` | Offset inside the group of a `repeat` table, from 0 to `step - 1`. Needs `repeat`. |
| `segment_index` | `true` reads the index of the segment instead of a value. Cannot take `component`. |
| `segment` | Id of the segment to read. A column of a table without `segment` names `segment` or `occurrence`. |
| `where` | Conditions on its elements: `{"<position>": "<value>"}`; with `segment`. |
| `occurrence` | Name of an occurrence of the reading loop, instead of `segment`; it excludes `segment` and `where`. |
| `loop` | The loop to read from, inside the anchor or above it; the anchor loop when absent. |
| `pick` | Which match to take: `"first"` (the default), `"last"` or a position counted from 1. Needs `occurrence`, on an occurrence that can repeat (not `max: 1`); a position cannot pass its `max`. |
| `phi` | `"name"`, `"id"` or `"date"`: the column holds patient data, hidden by a mask. `name` and `id` need a text column, `date` a date column; refused on an envelope control or count element. |

In a table with `segment`, the column reads that segment only, so `segment`, `where`,
`occurrence`, `loop` and `pick` are rejected. See [Choose where a column
reads](#choose-where-a-column-reads) and [Flag patient data](#flag-patient-data).

### `balancing.<name>`

A rule holds when, in every instance of the `per` loop, both sides add up to the same amount
to the last decimal place. A failure is reported as a diagnostic by `check/balance.rs`.

| Key | Required | Meaning | Rust |
|-----|----------|---------|------|
| `per` | yes | The loop whose every instance is checked. | `BalanceRule::per` |
| `target` | yes | The amounts the rule expects: a list of values. | `BalanceTerm` |
| `sum` | yes | The amounts that must add up to the target: a list of values. | `BalanceTerm` |

A value of `target` or `sum` takes `occurrence` (required, the name of an occurrence),
`elements` (required, positions to read), `loop` (optional, a loop below `per`; `per` itself
when absent), `component` (optional) and `sign` (`"+"`, the default, or `"-"`). Every
appearance of the occurrence adds to its side.

### `summaries.<name>` and `default_table`

A summary holds exactly one operation over a table of the spec: `count` (its rows),
`distinct` (the distinct non-null values of a column, compared as text), `sum` (a column
that reads a decimal `R` element; null cells are skipped) or `row` (named fields of the
table's one row). Every table and column a summary names must be declared in `tables`, so a
patch that removes one must also remove the summary (with `null`), or it fails to load with
an error that names the summary. `default_table` names one of the tables. The built-in 835
declares:

```json
"summaries": {
  "count_claims":   {"count":    {"table": "claims"}},
  "count_patients": {"distinct": {"table": "claims", "column": "patient_id"}},
  "sum_payments":   {"sum":      {"table": "payments", "column": "total_payment_amount"}},
  "payer": {"row": {"table": "payments", "fields": [
    {"name": "name", "column": "payer_name"},
    {"name": "identification_code", "column": "payer_id"},
    {"name": "address", "column": "payer_address"},
    {"name": "city", "column": "payer_city"},
    {"name": "state", "column": "payer_state"},
    {"name": "zip_code", "column": "payer_zip"}
  ]}},
  "payee": {"row": {"table": "payments", "fields": [
    {"name": "name", "column": "payee_name"},
    {"name": "identification_code", "column": "payee_id"},
    {"name": "address", "column": "payee_address"},
    {"name": "city", "column": "payee_city"},
    {"name": "state", "column": "payee_state"},
    {"name": "zip_code", "column": "payee_zip"}
  ]}}
},
"default_table": "claims"
```

| Key | Meaning | Rust |
|-----|---------|------|
| `count.table` | The table whose rows are counted. | `Summary::Count` |
| `distinct.table`, `distinct.column` | The column whose distinct values are counted. | `Summary::Distinct` |
| `sum.table`, `sum.column` | The decimal column that is added up. | `Summary::Sum` |
| `row.table`, `row.fields` | The table with one row, and the fields read from it: each `name` (the key the reader gives the value, unique) and `column`. | `Summary::Row` |
