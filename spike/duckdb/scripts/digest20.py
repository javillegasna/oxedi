import sys, pathlib, duckdb
ext, samples = sys.argv[1], pathlib.Path(sys.argv[2])
con = duckdb.connect(config={"allow_unsigned_extensions": "true"})
con.execute(f"LOAD '{ext}'")
print("duckdb", duckdb.__version__)
for p in sorted(samples.iterdir()):
    if p.suffix.lower() != ".rmt": continue
    for t in ["claims","payments","services","adjustments","provider_adjustments"]:
        r = con.execute(f"SELECT count(*), md5(string_agg(t::VARCHAR, '|' ORDER BY t.\"row\")) FROM read_835('{p}', table_name := '{t}') t").fetchone()
        print(f"{p.name:40} {t:22} rows={r[0]:>6} ext={str(r[1])[:12]}")
con.execute("CREATE TABLE c AS SELECT * FROM read_835(?)", [str(samples/'edi835_test_united.rmt')])
print(con.execute("SELECT * FROM oxedi835_rows_of('c')").fetchall())
con.execute("COPY c TO '/tmp/probe20.out' (FORMAT oxedi835_probe)")
print(open('/tmp/probe20.out').read().strip())
