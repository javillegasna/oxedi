# DuckDB extension

The `oxedi` extension reads X12 EDI 835 remittance files as DuckDB tables, and writes tables
back into an 835, with the same parser, writer and tables as the Python package. Every DuckDB
client (Python, R, Java, Node, Go, .NET, Rust, the CLI) gets it. It is built on DuckDB's stable
C API and loads on DuckDB 1.5.6 and later.

## Install

```sql
INSTALL oxedi FROM community;
LOAD oxedi;
```

The extension was accepted into DuckDB's community repository
(duckdb/community-extensions#2937), but its first community build failed on Windows on a test
that pinned the operating system's error text. The fixed release (0.1.2, PR #135) is built and
green and waits for the maintainers to merge duckdb/community-extensions#2942. Until then
`INSTALL oxedi FROM community` is not available: build the extension (see
[CONTRIBUTING](../CONTRIBUTING.md#duckdb-extension)) and load the file.

## Reading

```sql
FROM read_835('remittance.835');                                   -- claims (the default table)
FROM read_835('remits/*.835', table_name := 'services', filename := true);
FROM read_835(['a.835', 'b.835'], table_name := 'payments');

-- the findings of the parse, one row each
FROM read_835('remits/*.835', table_name := 'diagnostics', ignore_errors := true);

COPY (SELECT * FROM read_835('remits/*.835', table_name := 'claims')) TO 'claims.parquet';
```

`read_835(path, ...)` takes a path, a glob pattern or a list of them, and returns one table per
call. Parameters:

| Parameter | Default | Meaning |
|---|---|---|
| `table_name` | `'claims'` | `payments`, `claims`, `services`, `adjustments`, `provider_adjustments` or `diagnostics` |
| `filename` | `false` | add a `filename` column with the path each row came from |
| `version` | the file's own | force the built-in spec, `'5010'` or `'4010'`; by default each file uses the version it declares (5010 if none) |
| `binary` | `false` | return text columns as `BLOB` (the raw bytes) instead of `VARCHAR` |
| `ignore_errors` | `false` | report a file that is not an X12 interchange as a `diagnostics` row and go on |

To see the columns of a table, read any file:
`DESCRIBE SELECT * FROM read_835('remittance.835', table_name := 'claims')` lists the names and
types. A schema that does not need a file is tracked in
[#138](https://github.com/javillegasna/oxedi/issues/138).

### Text, VARCHAR and BLOB

Text columns are `VARCHAR`, so a field that is not valid UTF-8 fails the query with an error
that names the file and the cell. Pass `binary := true` to read those fields byte for byte
(nothing is lost to an encoding guess). The other columns are typed as in the Python tables:
decimals, dates and times.

### Files that are not interchanges

Without `ignore_errors`, a file that is not an interchange fails the query. With it, the file
contributes no rows to any table but one `diagnostics` row whose `rule` is the error, so run
the query with `table_name := 'diagnostics'` as well to see what was skipped. In that row
`datum` holds the bytes found instead of `ISA`: as escaped ASCII text (other bytes written as `\xNN`, always valid UTF-8) by
default, and raw with `binary := true`.

### Globs and remote paths

Glob patterns are expanded by a private in-memory DuckDB that honours your settings.
`enable_external_access` and `disabled_filesystems` apply: with `enable_external_access` off, a
glob pattern is an error. A remote pattern such as `s3://bucket/*.835` sees persistent secrets
only, not temporary `CREATE SECRET` ones. Plain paths and lists are read through your own file
system and secrets.

## Writing

Writing with `COPY` needs extension 0.2.0 or later; until it reaches the community repository,
build the extension.

`COPY ... TO ... (FORMAT edi835, ...)` writes an 835 with the same writer as the Python
package's `oxedi.write`; the file is byte for byte what `oxedi.write` gives for the same
tables and envelope. The query returns **one `STRUCT` column** whose fields are named after the
spec's tables, each a list of structs (one struct per row, the table's columns as fields):

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

Here `payments`, `claims` and the rest are tables or views with those names, for example
`CREATE TEMP TABLE claims AS FROM read_835('remittance.835', table_name := 'claims')`, changed
in SQL before writing. Temporary tables, views and the open transaction are visible. The lists of
several rows of the query concatenate into one interchange. A table left out has no rows and a
column left out is null.

To write from tables with your own keys, see
[From your own tables](writing.md#with-sql-duckdb).

### Options

| Option | Default | Meaning |
|---|---|---|
| `sender_id`, `receiver_id` | required | interchange sender and receiver ids |
| `date`, `time` | required | the interchange date (`DATE`) and time (`TIME` or `TIME_NS`) |
| `sender_qualifier`, `receiver_qualifier` | `'ZZ'` | ISA qualifiers |
| `usage_indicator` | `'P'` | `'P'` production or `'T'` test |
| `control_number` | `1` | first interchange control number |
| `application_sender`, `application_receiver` | the ids | group header sender and receiver codes |
| `delimiters` | `*` `:` `~` `^` | a struct such as `{'element': '\|', 'repetition': '^'}` with any of `element`, `component`, `segment`, `repetition`, `release`; fields left out take the defaults of `oxedi.Delimiters()` (`*`, `:`, `~`, and no repetition or release), so on 5010 give `repetition` |
| `line_break` | `false` | a line break after each segment |
| `version` | `'5010'` | the built-in spec, `'5010'` or `'4010'` |

The options are those of `oxedi.Envelope`, with the same defaults.

### Strict

The written file is read back with the spec, and every diagnostic of that read is a finding, as
is anything the writer cannot place (a missing required element, a code outside its list, a row
whose parent does not exist or is out of order, money that does not balance). Any finding fails
the `COPY` with the listing `oxedi.WriteError` shows, each finding naming the table, row and
column, and nothing is written. There is no `allow_findings` here yet.

### Accepted types

Per column of the spec:

- Text takes `VARCHAR`, `ENUM` or `BLOB`.
- An integer column takes any integer type, or a `FLOAT` or `DOUBLE` holding a whole number.
- A decimal column takes any `DECIMAL` or integer type that holds the value exactly (never
  `FLOAT` or `DOUBLE`).
- A date column takes `DATE`, or `TIMESTAMP` at midnight.
- A time column takes `TIME` or `TIME_NS` in whole seconds.

### From your own tables

The writer asks for the spec's names and nothing else, so tables built elsewhere work when they
follow them (see [Writing](writing.md#from-your-own-tables)):

- **Tolerated:** a missing table (no rows), a missing column (null), a column of a narrower or
  compatible type (`INTEGER` for `BIGINT`, a `DECIMAL` of another scale that holds the value,
  `ENUM` or `BLOB` for text).
- **Refused, naming the table, row and column:** a table or column that is not in the spec, a float
  for money, a value that would change in the conversion (a decimal that does not fit, a number
  outside the column's range), and a file that does not balance or is missing a required
  element.

### Limits

- `PARTITION_BY` and `PER_THREAD_OUTPUT` ask for several files; DuckDB consumes them and the
  format refuses a second file. `compression` is refused too: compress afterwards.
- An existing file is left untouched when the `COPY` fails only for a local path written with
  DuckDB's default temporary file. Without it (`USE_TMP_FILE false`, or a path that is not
  local) the file is written over from its first byte, an existing file longer than the new
  content is refused ("remove it first") and what happens to the target of a failed `COPY` is
  up to DuckDB (some versions remove it).
- A bare `NULL` inside a struct is typed `INTEGER` by DuckDB and a text column refuses it;
  write `NULL::VARCHAR` (or the column's type).

## Versions

The extension is versioned separately from the Python package (its version is in the crate's
`Cargo.toml`), and its releases are tagged `duckdb-v*`. Its own changelog is
[`crates/oxedi_duckdb/CHANGELOG.md`](../crates/oxedi_duckdb/CHANGELOG.md).
