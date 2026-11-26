# oxedi in a pipeline

A pipeline step that receives an 835, checks it and hands it on needs four moves: parse the
bytes, look at the diagnostics, keep or reject, and write the result out.

## The step

```python
import oxedi
from pathlib import Path


def process(data: bytes, out_dir: Path, name: str) -> list:
    """Parse one file; write its tables as Parquet; return its problems."""
    try:
        result = oxedi.parse(data)
    except oxedi.ParseError as error:  # not an 835 at all
        return [str(error)]

    problems = [str(d) for d in result.diagnostics]
    if problems:
        return problems  # leave the file for a person; write nothing

    out_dir.mkdir(parents=True, exist_ok=True)
    for table in result.tables.keys():
        result.tables[table].to_polars().write_parquet(out_dir / f"{name}-{table}.parquet")
    return []


problems = process(Path("remittance.835").read_bytes(), Path("out"), "remittance")
print("clean" if not problems else problems)
```

`parse` never raises on questionable data, only on a file that is not an 835. Diagnostics are
the signal. To triage, filter on `level` (the SNIP level) or `rule`. See
[Diagnostics](../python/diagnostics.md).

The step has no global state and no I/O of its own beyond what you pass in, so it runs the same
in a script, a task or a consumer.

## Keep the original bytes

A parsed file writes back exactly as it came in. Archive that alongside the tables, so the
source of every number is one call away:

```python
result = oxedi.parse_file("remittance.835")
assert result.document.write() == Path("remittance.835").read_bytes()
```

## Airflow

Wrap `process` in a task. Pass paths, not bytes, between tasks. Not executed here, because it
needs an Airflow installation:

<!-- docs-test: skip needs Airflow; the step it calls runs above -->
```python
from airflow.sdk import task  # Airflow 2: from airflow.decorators import task

@task
def load_remittance(path: str) -> list:
    return process(Path(path).read_bytes(), Path("/data/out"), Path(path).stem)
```

## Dagster

An asset with the problems as metadata. Not executed here, because it needs
Dagster:

<!-- docs-test: skip needs Dagster; the step it calls runs above -->
```python
from dagster import Config, Output, asset

class RemitConfig(Config):
    path: str

@asset
def remittance_tables(context, config: RemitConfig):
    path = config.path
    problems = process(Path(path).read_bytes(), Path("/data/out"), Path(path).stem)
    return Output(None, metadata={"problems": len(problems)})
```

## Kafka

Treat each message as one file: call `process` on the message value and send a message with
problems to a dead-letter topic when the list is not empty. Commit the offset only after the
output is written, so a crash re-processes the message rather than losing it. `oxedi.stream(data)` yields one
transaction at a time when a single message is large.

## Alongside SQL

If the files land in object storage, the same checks run in DuckDB without Python. See
[a Parquet lakehouse](lakehouse.md).
