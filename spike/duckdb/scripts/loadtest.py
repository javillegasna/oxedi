import sys, duckdb
ext, sample = sys.argv[1], sys.argv[2]
con = duckdb.connect(config={"allow_unsigned_extensions": "true"})
try:
    con.execute(f"LOAD '{ext}'")
    print(duckdb.__version__, ext.split('/')[-2], "LOADED", con.execute(f"SELECT count(*), sum(charge_amount) FROM read_835('{sample}')").fetchone())
except Exception as e:
    print(duckdb.__version__, ext.split('/')[-2], "FAILED:", str(e).splitlines()[0][:220])
