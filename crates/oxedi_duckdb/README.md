# oxedi, the DuckDB extension

`oxedi` reads X12 EDI 835 remittance files as DuckDB tables, with the same parser and the
same tables as the [oxedi835](../../README.md) Python package. Every DuckDB client (Python,
R, Java, Node, Go, .NET, Rust, the CLI) gets it. It is built on DuckDB's stable C API and
loads on DuckDB 1.5.6 and later. It is not yet published to the community repository, so
`INSTALL oxedi FROM community` works only once it is; until then build it (below) and
`LOAD` the file.

```sql
INSTALL oxedi FROM community;
LOAD oxedi;

FROM read_835('remittance.835');                                   -- claims (the default table)
FROM read_835('remits/*.835', table_name := 'services', filename := true);
FROM read_835(['a.835', 'b.835'], table_name := 'payments');

-- the findings of the parse, one row each
FROM read_835('remits/*.835', table_name := 'diagnostics', ignore_errors := true);

COPY (SELECT * FROM read_835('remits/*.835', table_name := 'claims')) TO 'claims.parquet';
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

## Build and test

From the repository root (Rust, Python 3 and the `extension-ci-tools` submodule are needed):

```bash
git submodule update --init
make configure_ci          # test venv, platform and version (the workspace's, as v<version>)
make release               # build/release/extension/oxedi/oxedi.duckdb_extension
make test_release          # SQLLogicTest files in crates/oxedi_duckdb/test/sql
make duckdb-oracle         # read_835 against oxedi835.parse_file, row by row
```

Load a local build with `duckdb -unsigned` and `LOAD 'build/release/extension/oxedi/oxedi.duckdb_extension';`.
`description.yml` is the descriptor for the community repository.
