# Reading large files

Read a file one transaction or one claim at a time, so the tables of each step stay small.

## Stream one transaction at a time

```python
import oxedi
from pathlib import Path

for batch in oxedi.stream(Path("big.835").read_bytes()):
    services = batch.tables["services"].to_polars()
    print(batch)
```

```text
Batch(tables=Tables(adjustments: 15 rows, claims: 4 rows, payments: 1 rows, provider_adjustments: 3 rows, services: 4 rows), diagnostics=0)
```

- Each batch holds the tables and diagnostics of one transaction: the rows and findings since
  the previous batch.
- The example file has one transaction, so it gives one batch.
- `stream` accepts `spec=` like `parse`.

## Stream one claim at a time

Pass `by="2100"` (or any other loop id) to get a batch each time a claim closes:

```python
for batch in oxedi.stream(Path("big.835").read_bytes(), by="2100"):
    print(len(batch.tables["claims"]), len(batch.tables["services"]), len(batch.tables["payments"]))
```

```text
1 2 0
1 1 0
1 1 0
1 0 0
0 0 1
```

A row is added to its table when its loop closes. So the payment, which closes after its
claims, comes in the last batch. Row numbers count across the whole file, so `claim` and
`payment` still point at the right rows from one batch to the next.

## Parse files in parallel

Parsing releases the GIL, so several files can be parsed in parallel from threads:

```python
from concurrent.futures import ThreadPoolExecutor

paths = sorted(Path("remits").glob("*.835"))
with ThreadPoolExecutor() as pool:
    results = list(pool.map(oxedi.parse_file, paths))

print([result.summary("count_claims") for result in results])
```

```text
[4, 4, 3]
```

!!! warning "One thread per stream"
    A single stream must be advanced from one thread at a time. A `next()` that finds another
    thread inside a step raises `RuntimeError`.
