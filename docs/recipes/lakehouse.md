# A Parquet lakehouse with DuckDB

Turn a folder of 835 files into partitioned Parquet that any engine can query. All of it is SQL
in DuckDB with the `oxedi` extension (see [DuckDB](../duckdb/index.md) to install it).

## Read a folder

`read_835` takes a path, a glob or a list. Add `filename := true` to know which file each row
came from. Row numbers restart in every file, so a child table joins its parent on the file
name and the row number.

```sql
SELECT filename, count(*) AS claims, sum(payment_amount) AS paid
FROM read_835('remits/*.835', table_name := 'claims', filename := true)
GROUP BY filename
ORDER BY filename;
```

## Write Parquet partitioned by payer and month

One wide table of claims, with the payer and the payment date from the payment each claim
belongs to, is the usual starting point.

```sql
CREATE TABLE claims_wide AS
SELECT
    p.payer_name                              AS payer,
    strftime(p.payment_date, '%Y-%m')         AS month,
    p.trace_number,
    c.claim_id,
    c.claim_status,
    c.charge_amount,
    c.payment_amount,
    c.patient_responsibility,
    c.filename
FROM read_835('remits/*.835', table_name := 'claims', filename := true) c
JOIN read_835('remits/*.835', table_name := 'payments', filename := true) p
  ON p.filename = c.filename AND p."row" = c.payment;

COPY claims_wide TO 'claims_lake' (FORMAT parquet, PARTITION_BY (payer, month), OVERWRITE_OR_IGNORE);
```

The output has one folder per payer and month, such as
`claims_lake/payer=ACME%20HEALTH%20PLAN/month=2024-01/` (values are URL-encoded). Read it back, with the partition values as
columns:

```sql
SELECT payer, month, count(*) AS claims, sum(payment_amount) AS paid
FROM read_parquet('claims_lake/**/*.parquet', hive_partitioning = true)
GROUP BY ALL
ORDER BY ALL;
```

Do the same for `services` and `adjustments` and join them on `filename` and the row
references, as in the query above.

## Use diagnostics as a quality gate

Every file parses, and its problems come back as rows of the `diagnostics` table. Count them
before you publish a batch. `ignore_errors := true` turns a file that is not an 835 into a
row, so one bad file does not stop the run.

```sql
SELECT filename, level, rule, count(*) AS diagnostics
FROM read_835('remits/*.835', table_name := 'diagnostics', filename := true, ignore_errors := true)
GROUP BY ALL
ORDER BY filename, level;
```

An empty result means the batch is clean. To stop a script at the first problem, make the
count fail the statement:

```sql
SELECT CASE WHEN count(*) = 0 THEN 'clean'
            ELSE error('835 diagnostics found: ' || count(*)) END AS gate
FROM read_835('remits/*.835', table_name := 'diagnostics', ignore_errors := true);
```

## dbt sources over `read_835`

With [dbt-duckdb](https://github.com/duckdb/dbt-duckdb), load the extension in the profile and
build one staging model per table. The profile (not executed here):

```yaml
oxedi_lake:
  target: dev
  outputs:
    dev:
      type: duckdb
      path: lake.duckdb
      extensions:
        - name: oxedi
          repo: community
```

A staging model, `models/staging/stg_claims.sql`:

<!-- docs-test: skip dbt renders {{ config(...) }} and {{ var(...) }}; the equivalent SQL below runs -->
```sql
{{ config(materialized='view') }}

SELECT *
FROM read_835('{{ var("remits_glob", "remits/*.835") }}', table_name := 'claims', filename := true)
```

A dbt singular test fails when it returns rows, so a test that selects the diagnostics fails
the build on any diagnostic:

<!-- docs-test: skip dbt renders {{ var(...) }}; the equivalent SQL below runs -->
```sql
-- tests/no_835_diagnostics.sql: a dbt singular test passes when it returns no rows
SELECT *
FROM read_835('{{ var("remits_glob", "remits/*.835") }}', table_name := 'diagnostics', filename := true)
```

The same two statements without the template, which is what dbt sends to DuckDB:

```sql
CREATE OR REPLACE VIEW stg_claims AS
SELECT *
FROM read_835('remits/*.835', table_name := 'claims', filename := true);

SELECT count(*) AS claims, sum(payment_amount) AS paid FROM stg_claims;

SELECT *
FROM read_835('remits/*.835', table_name := 'diagnostics', filename := true);
```
