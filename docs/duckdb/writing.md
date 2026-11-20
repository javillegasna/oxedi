# Writing with COPY

`COPY ... TO ... (FORMAT edi835, ...)` writes an 835 from SQL. It uses the same writer as the
Python package's `oxedi.write`. For the same tables and envelope, the file is byte for byte
what `oxedi.write` gives.

!!! warning "Needs extension 0.2.0"
    Writing with `COPY` needs extension 0.2.0 or later. Until it reaches the community
    repository, build the extension (see [Install](index.md#install)).

## Write a file

The query you copy returns **one `STRUCT` column**:

- Its fields are named after the spec's tables.
- Each field is a list of structs: one struct per row, with the table's columns as fields.

This example reads the tables of a file into temporary tables:

```sql
CREATE TEMP TABLE payments AS FROM read_835('remittance.835', table_name := 'payments');
CREATE TEMP TABLE claims AS FROM read_835('remittance.835', table_name := 'claims');
CREATE TEMP TABLE services AS FROM read_835('remittance.835', table_name := 'services');
CREATE TEMP TABLE adjustments AS FROM read_835('remittance.835', table_name := 'adjustments');
CREATE TEMP TABLE provider_adjustments AS
  FROM read_835('remittance.835', table_name := 'provider_adjustments');
```

Then it writes them back:

```sql
COPY (
  SELECT {
    'payments':             (SELECT list(t ORDER BY t."row") FROM payments t),
    'claims':               (SELECT list(t ORDER BY t."row") FROM claims t),
    'services':             (SELECT list(t ORDER BY t."row") FROM services t),
    'adjustments':          (SELECT list(t ORDER BY t."row") FROM adjustments t),
    'provider_adjustments': (SELECT list(t ORDER BY t."row") FROM provider_adjustments t)
  }
) TO 'out.835' (FORMAT edi835, sender_id 'ACMEPAYER', receiver_id 'SUNRISECLINIC',
                date DATE '2024-01-10', time TIME '09:00');
```

The new file reads back like any other:

```sql
SELECT count(*) AS claims, sum(payment_amount) AS paid FROM read_835('out.835');
```

```text
┌────────┬───────────────┐
│ claims │     paid      │
│ int64  │ decimal(38,2) │
├────────┼───────────────┤
│      4 │        720.00 │
└────────┴───────────────┘
```

- `payments`, `claims` and the rest can be any tables or views with those names. Build them in
  SQL, or read them from a file as above and change them before writing.
- Temporary tables, views and the open transaction are visible.
- The lists of several rows of the query concatenate into one interchange.
- A table left out has no rows. A column left out is null.

## Options

| Option | Default | Meaning |
|---|---|---|
| `sender_id`, `receiver_id` | required | interchange sender and receiver ids |
| `date`, `time` | required | the interchange date (`DATE`) and time (`TIME` or `TIME_NS`) |
| `sender_qualifier`, `receiver_qualifier` | `'ZZ'` | ISA qualifiers |
| `usage_indicator` | `'P'` | `'P'` production or `'T'` test |
| `control_number` | `1` | first interchange control number |
| `application_sender`, `application_receiver` | the ids | group header sender and receiver codes |
| `delimiters` | `*` `:` `~` `^` | a struct of delimiters (see below) |
| `line_break` | `false` | a line break after each segment |
| `version` | `'5010'` | the built-in spec, `'5010'` or `'4010'` |

The options are those of [`oxedi.Envelope`](../python/writing.md#fill-in-the-envelope), with the
same defaults.

??? note "Details: the `delimiters` struct"
    Pass a struct such as `{'element': '|', 'repetition': '^'}` with any of `element`,
    `component`, `segment`, `repetition` and `release`. Fields left out take the defaults of
    `oxedi.Delimiters()`: `*`, `:`, `~`, and no repetition or release. So on 5010, give
    `repetition`.

    With a `release` character, a value holding the element separator, the component
    separator, the segment terminator or the release character is written with the release
    character before it, for example `A?*B`, instead of being a finding. The values of the
    interchange header and a value holding the repetition separator are still findings. The
    file cannot be read back with `read_835`, which has no release option. Python's
    `oxedi.parse(data, delimiters=oxedi.Delimiters(repetition=b"^", release=b"?"))` reads it.

## When COPY refuses

`COPY` is strict, like `oxedi.write`. The written file is read back with the spec. Every
diagnostic of that read is a finding, and so is anything the writer cannot place:

- A missing required element.
- A code outside its list.
- A row whose parent does not exist or is out of order.
- Money that does not balance.

Any finding fails the `COPY` with the listing `oxedi.WriteError` shows, each finding naming the
table, row and column. Nothing is written. There is no `allow_findings` here yet.
[When the writer refuses a file](../python/when-write-refuses.md) explains findings in detail.

## Write from your own tables

The writer asks for the spec's names and nothing else. Tables built elsewhere work when they
follow them.

What the writer tolerates and what it refuses is listed in
[What the writer accepts](../python/own-tables.md#what-the-writer-accepts). It is the same for
`COPY`, with `INTEGER` for `BIGINT` and `ENUM` or `BLOB` for text as the SQL types.

`oxedi_schema()` lists the columns and types of each table, and which columns refer to which
table, without a file; see [See the columns without a file](reading.md#see-the-columns-without-a-file).

To number rows from tables with your own keys, see
[Writing from your tables](../python/own-tables.md#translate-your-keys) (SQL tab).
[Types and limits](limits.md) lists the accepted types per column.
