from conftest import GOLDEN, SUMMARY_ONLY, parse_named


def test_tables_render_as_the_golden_files(file_name):
    tables = parse_named(file_name).tables
    if file_name in SUMMARY_ONLY:
        actual = "".join(f"{name} rows: {len(tables[name])}\n" for name in tables)
        golden = GOLDEN / f"{file_name}.tables.summary.txt"
    else:
        actual = tables.render()
        golden = GOLDEN / f"{file_name}.tables.txt"
    assert actual == golden.read_text(encoding="utf-8")


def test_diagnostics_display_as_the_golden_files(file_name):
    diagnostics = parse_named(file_name).diagnostics
    golden = GOLDEN / f"{file_name}.diagnostics.txt"
    assert "".join(f"{d}\n" for d in diagnostics) == golden.read_text(
        encoding="utf-8"
    )
