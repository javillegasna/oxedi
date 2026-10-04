"""Order-of-magnitude timing: read_835 in DuckDB against parse_file + from_arrow.

usage: python bench.py EXTENSION_PATH SAMPLE [RUNS]
"""

import statistics
import sys
import time

import duckdb
import oxedi835
import pyarrow as pa


def timed(fn, runs):
    fn()
    samples = []
    for _ in range(runs):
        start = time.perf_counter()
        fn()
        samples.append(time.perf_counter() - start)
    return statistics.median(samples) * 1000


def main() -> None:
    extension, sample = sys.argv[1], sys.argv[2]
    runs = int(sys.argv[3]) if len(sys.argv) > 3 else 30
    con = duckdb.connect(config={"allow_unsigned_extensions": "true"})
    con.execute(f"LOAD '{extension}'")
    query = "SELECT count(*), sum(charge_amount) FROM {src}"

    def ext_aggregate():
        con.execute(query.format(src=f"read_835('{sample}')")).fetchall()

    def ext_materialize():
        con.execute(f"SELECT * FROM read_835('{sample}')").to_arrow_table()

    def py_parse_only():
        oxedi835.parse_file(sample)

    def py_aggregate():
        claims = pa.table(oxedi835.parse_file(sample).tables["claims"])
        con.from_arrow(claims).aggregate("count(*), sum(charge_amount)").fetchall()

    def py_materialize():
        claims = pa.table(oxedi835.parse_file(sample).tables["claims"])
        con.from_arrow(claims).to_arrow_table()

    print(f"duckdb {duckdb.__version__}, median of {runs} runs, ms")
    for name, fn in [
        ("read_835 -> count/sum", ext_aggregate),
        ("read_835 -> arrow table", ext_materialize),
        ("parse_file only", py_parse_only),
        ("parse_file + from_arrow -> count/sum", py_aggregate),
        ("parse_file + from_arrow -> arrow table", py_materialize),
    ]:
        print(f"{name:42} {timed(fn, runs):8.2f}")


if __name__ == "__main__":
    main()
