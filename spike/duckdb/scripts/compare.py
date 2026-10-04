"""Compare read_835 (DuckDB extension) with oxedi835.parse_file through pyarrow.

usage: python compare.py EXTENSION_PATH SAMPLES_DIR
Prints, per sample and table, the row count and an md5 over every row rendered
as text, from both sides, and whether they match.
"""

import pathlib
import sys

import duckdb
import oxedi835
import pyarrow as pa

TABLES = ["claims", "payments", "services", "adjustments", "provider_adjustments"]
DIGEST = "SELECT count(*), md5(string_agg(t::VARCHAR, '|' ORDER BY t.\"row\")) FROM {src} t"


def main() -> int:
    extension, samples = sys.argv[1], pathlib.Path(sys.argv[2])
    con = duckdb.connect(config={"allow_unsigned_extensions": "true"})
    con.execute(f"LOAD '{extension}'")
    print("duckdb", duckdb.__version__)
    mismatches = 0
    for path in sorted(samples.iterdir()):
        if path.suffix.lower() != ".rmt":
            continue
        result = oxedi835.parse_file(str(path))
        for table in TABLES:
            ours = con.execute(
                DIGEST.format(src=f"read_835('{path}', table_name := '{table}')")
            ).fetchone()
            arrow = pa.table(result.tables[table])  # noqa: F841 (read by DuckDB's replacement scan)
            theirs = con.execute(DIGEST.format(src="arrow")).fetchone()
            same = ours == theirs
            mismatches += not same
            print(f"{path.name:40} {table:22} rows={ours[0]:>6} ext={str(ours[1])[:12]} py={str(theirs[1])[:12]} {'OK' if same else 'MISMATCH'}")
    print("mismatches:", mismatches)
    return 1 if mismatches else 0


if __name__ == "__main__":
    sys.exit(main())
