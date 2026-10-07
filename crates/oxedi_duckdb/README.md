# oxedi, the DuckDB extension

`oxedi` reads X12 EDI 835 remittance files as DuckDB tables, and writes tables back into an
835, with the same parser, writer and tables as the [oxedi](../../README.md) Python package.
Every DuckDB client (Python, R, Java, Node, Go, .NET, Rust, the CLI) gets it. It loads on
DuckDB 1.5.6 and later.

## Install

```sql
INSTALL oxedi FROM community;
LOAD oxedi;
```

The extension installs from DuckDB's community repository on DuckDB 1.5.6 or later.

## Use

```sql
FROM read_835('remittance.835');                                   -- claims (the default table)
FROM read_835('remits/*.835', table_name := 'services', filename := true);

-- the findings of the parse, one row each
FROM read_835('remits/*.835', table_name := 'diagnostics', ignore_errors := true);
```

Write tables back into an 835 with `COPY` (extension 0.2.0 or later). `payments`, `claims` and the rest are tables or
views of those names:

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

Parameters, options, types, limits and writing from your own tables are in the
[DuckDB guide](../../docs/duckdb.md).

## Versions

The extension is versioned separately from the Python package (its version is in this crate's
`Cargo.toml`), and its releases are tagged `duckdb-v*`. See [`CHANGELOG.md`](CHANGELOG.md).

## Build and test

From the repository root (Rust, Python 3 and the `extension-ci-tools` submodule are needed):

```bash
git submodule update --init
make configure_ci          # test venv, platform and version (the crate's, as v<version>)
make release               # build/release/extension/oxedi/oxedi.duckdb_extension
make test_release          # SQLLogicTest files in crates/oxedi_duckdb/test/sql
make duckdb-oracle         # read_835 against oxedi.parse_file, row by row
```

Load a local build with `duckdb -unsigned` and
`LOAD 'build/release/extension/oxedi/oxedi.duckdb_extension';`. `description.yml` is the
descriptor for the community repository.
