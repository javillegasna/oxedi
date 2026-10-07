# oxedi, the DuckDB extension

`oxedi` reads X12 EDI 835 remittance files as DuckDB tables, and writes tables back into an
835, with the same parser, writer and tables as the [oxedi](../../README.md) Python package. Every DuckDB client (Python,
R, Java, Node, Go, .NET, Rust, the CLI) gets it. It is built on DuckDB's stable C API and
loads on DuckDB 1.5.6 and later. It is accepted in the community repository but not
installable yet: its first community build failed on Windows on a test that pinned the
operating system's error text, and the fixed release (0.1.2, PR #135) is built and green and waits for the maintainers to merge
duckdb/community-extensions#2942. Until then build it
(below) and `LOAD` the file.

```sql
INSTALL oxedi FROM community;
LOAD oxedi;

FROM read_835('remittance.835');                                   -- claims (the default table)
FROM read_835('remits/*.835', table_name := 'services', filename := true);
FROM read_835(['a.835', 'b.835'], table_name := 'payments');

-- the findings of the parse, one row each
FROM read_835('remits/*.835', table_name := 'diagnostics', ignore_errors := true);

COPY (SELECT * FROM read_835('remits/*.835', table_name := 'claims')) TO 'claims.parquet';

-- write tables back into an 835 (see Writing below)
COPY (SELECT {'payments': (SELECT list(t ORDER BY t."row") FROM payments t), ...})
  TO 'out.835' (FORMAT edi835, sender_id 'ACMEPAYER', receiver_id 'SUNRISECLINIC',
                date DATE '2024-01-10', time TIME '09:00');
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

## Writing

`COPY ... TO ... (FORMAT edi835, ...)` writes an 835 with the same writer as the Python
package's `oxedi.write`; the file is byte for byte what `oxedi.write` gives for the same tables
and envelope. The query returns **one `STRUCT` column** whose fields are named after the spec's
tables, each a list of structs (one struct per row, the table's columns as fields):

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

Here `payments`, `claims` and the rest are tables or views of the same names, for example
`CREATE TEMP TABLE claims AS FROM read_835('remittance.835', table_name := 'claims')` after a
change in SQL. Temporary tables, views and the open transaction are visible. The lists of several
rows of the query concatenate into one interchange. A table left out has no rows and a column left out is null.

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

**Strict.** The written file is read back with the spec, and every diagnostic of that read is a
finding, as is anything the writer cannot place (a missing required element, a code outside its
list, a row whose parent does not exist or is out of order, money that does not balance).
Any finding fails the `COPY` with the listing `oxedi.WriteError` shows, each finding naming the
table, row and column, and nothing is written. There is no `allow_findings` here yet.

**Limits.**
- `PARTITION_BY` and `PER_THREAD_OUTPUT` ask for several files; DuckDB consumes them and the
  format refuses a second file. `compression` is refused too: compress afterwards.
- An existing file is left untouched when the `COPY` fails only for a local path written
  with DuckDB's default temporary file. Without it (`USE_TMP_FILE false`, or a path that is not
  local) the file is written over from its first byte, an existing file longer than the new
  content is refused ("remove it first") and what happens to the target of a failed `COPY` is up
  to DuckDB (some versions remove it).
- A bare `NULL` inside a struct is typed `INTEGER` by DuckDB and a text column refuses it;
  write `NULL::VARCHAR` (or the column's type).
- Accepted types, per column of the spec: text takes `VARCHAR`, `ENUM` or `BLOB`; an integer
  column any integer type, or a `FLOAT` or `DOUBLE` holding a whole number; a decimal column
  any `DECIMAL` or integer type that holds the value exactly (never `FLOAT` or `DOUBLE`); a date
  column `DATE`, or `TIMESTAMP` at midnight; a time column `TIME` or `TIME_NS` in whole seconds.

### From your own tables

The writer asks for the spec's names and nothing else, so tables built elsewhere work when
they follow them.

- **Tolerated:** a missing table (no rows), a missing column (null), a column of a narrower or
  compatible type (`INTEGER` for `BIGINT`, a `DECIMAL` of another scale that holds the value,
  `ENUM` or `BLOB` for text; the accepted types are listed above).
- **Refused, naming the table and column:** a table or column that is not in the spec, a
  float for money, a value that would change in the conversion (a decimal that does not fit,
  a number outside the column's range), and a file that does not balance or is missing a
  required element.

The spec's tables key their rows by position: `row` is the row's ordinal in its table starting
at 0, and `payment`, `claim` and `service` hold the `row` of the parent. If your tables have
their own keys, give each table a `row` with `row_number()`, put the parents' `row` in its
parent columns with a join, and drop your helper columns with `EXCLUDE`. Rows of one parent
must be together and in their parents' order, so number each child table ordered by the
parent's `row` first, then by its own key, then by a stable tie-breaker (a sequence column, or
`rowid` of a table). The adjustments of a claim, which have no `service`, come before those of
its services (`NULLS FIRST`). Your keys need not sort in parent order across parents.
Keep the `segment` column of `provider_adjustments` when you have one: adjustments with the same
`segment` are written in one `PLB`, and without it all the adjustments of a payment share one `PLB`.

This example takes `my_payments` (key `pay_key`), `my_claims` (`claim_key`, `pay_key`),
`my_services` (`service_key`, `claim_key`), `my_adjustments` (`claim_key`, and `service_key`
when the adjustment is on a service) and `my_provider_adjustments` (`pay_key`), each with the
spec's other columns:

```sql
COPY (
  WITH payments AS (
         SELECT row_number() OVER (ORDER BY pay_key) - 1 AS "row", * FROM my_payments),
       claims AS (
         SELECT row_number() OVER (ORDER BY p."row", c.claim_key) - 1 AS "row", p."row" AS payment, c.*
         FROM my_claims c JOIN payments p USING (pay_key)),
       services AS (
         SELECT row_number() OVER (ORDER BY c."row", s.service_key) - 1 AS "row",
                c.payment, c."row" AS claim, s.*
         FROM my_services s JOIN claims c USING (claim_key)),
       adjustments AS (
         SELECT row_number() OVER (ORDER BY c."row", s."row" NULLS FIRST, a.rowid) - 1 AS "row",
                c.payment, c."row" AS claim, s."row" AS service, a.*
         FROM my_adjustments a
         JOIN claims c USING (claim_key)
         LEFT JOIN services s ON s.service_key = a.service_key),
       provider_adjustments AS (
         SELECT row_number() OVER (ORDER BY p."row", a.rowid) - 1 AS "row", p."row" AS payment, a.*
         FROM my_provider_adjustments a JOIN payments p USING (pay_key))
  SELECT {
    'payments': (SELECT list(t ORDER BY t."row") FROM (SELECT * EXCLUDE (pay_key) FROM payments) t),
    'claims': (SELECT list(t ORDER BY t."row")
               FROM (SELECT * EXCLUDE (pay_key, claim_key) FROM claims) t),
    'services': (SELECT list(t ORDER BY t."row")
                 FROM (SELECT * EXCLUDE (service_key, claim_key) FROM services) t),
    'adjustments': (SELECT list(t ORDER BY t."row")
                    FROM (SELECT * EXCLUDE (claim_key, service_key) FROM adjustments) t),
    'provider_adjustments': (SELECT list(t ORDER BY t."row")
                             FROM (SELECT * EXCLUDE (pay_key) FROM provider_adjustments) t)
  }
) TO 'out.835' (FORMAT edi835, sender_id 'ACMEPAYER', receiver_id 'SUNRISECLINIC',
                date DATE '2024-01-10', time TIME '09:00');
```

To see the columns of a table today, read any file: `DESCRIBE SELECT * FROM
read_835('remittance.835', table_name := 'claims')` lists the names and types (or
`Result.tables["claims"]` in Python). A file-free schema is tracked in
[#138](https://github.com/javillegasna/oxedi/issues/138).

## Versions

The extension is versioned separately from the Python package (its version is in this
crate's `Cargo.toml`), and its releases are tagged `duckdb-v*`.

## Build and test

From the repository root (Rust, Python 3 and the `extension-ci-tools` submodule are needed):

```bash
git submodule update --init
make configure_ci          # test venv, platform and version (the crate's, as v<version>)
make release               # build/release/extension/oxedi/oxedi.duckdb_extension
make test_release          # SQLLogicTest files in crates/oxedi_duckdb/test/sql
make duckdb-oracle         # read_835 against oxedi.parse_file, row by row
```

Load a local build with `duckdb -unsigned` and `LOAD 'build/release/extension/oxedi/oxedi.duckdb_extension';`.
`description.yml` is the descriptor for the community repository.
