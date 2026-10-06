"""read_835 (the DuckDB extension) against oxedi.parse_file (the Python binding).

For every sample, every fixture and every file of test/data, every table and
`diagnostics` must hold the same rows in the same order. Each row is rendered
canonically (the repr of each Python value: bytes, int, Decimal with its scale,
datetime.date, datetime.time or None) and hashed with md5; the per-row digests
must be equal. Text is compared as bytes (`binary := true`) and, when the
oracle's bytes are UTF-8, as VARCHAR too; a non-UTF-8 cell must fail the VARCHAR
query instead.

The extension is read from OXEDI_EXTENSION, else from this crate's release build.
"""

from __future__ import annotations

import hashlib
import os
import pathlib
from typing import Any, Iterable, Sequence

import duckdb
import oxedi
import pyarrow as pa
import pytest

CRATE = pathlib.Path(__file__).resolve().parents[2]
CORE_TESTS = CRATE.parent / "oxedi_core" / "tests"
EXTENSION = pathlib.Path(
    os.environ.get(
        "OXEDI_EXTENSION",
        CRATE / "build" / "release" / "extension" / "oxedi" / "oxedi.duckdb_extension",
    )
)
DIAGNOSTICS = "diagnostics"
DIAGNOSTIC_COLUMNS = [
    "level",
    "kind",
    "rule",
    "segment",
    "element",
    "component",
    "path",
    "datum",
    "origin",
    "code",
]
UTF8_ERROR = "a VARCHAR must be valid UTF-8"

# Column names and rows of one table.
Rows = tuple[list[str], list[tuple[Any, ...]]]


def input_files() -> list[pathlib.Path]:
    folders = [CORE_TESTS / "samples", CORE_TESTS / "fixtures", CRATE / "test" / "data"]
    return sorted(
        path
        for folder in folders
        for path in folder.iterdir()
        if path.is_file() and path.suffix.lower() != ".md"
    )


FILES = input_files()

# The inputs the core cannot parse, each with the bytes found instead of ISA:
# a fixture without an envelope and the extension's own non-835 file. Every
# other input must parse.
UNPARSABLE = {
    "fixtures/blue_cross_nc_sample.txt": "ST*835*1",
    "data/not_an_interchange.835": "hello, t",
    "data/not_text.835": r"\x1f\x8b\x08\x00\x00\x00\x00\x00",
}


def file_id(path: pathlib.Path) -> str:
    return f"{path.parent.name}/{path.name}"


@pytest.fixture(scope="module")
def con() -> Iterable[duckdb.DuckDBPyConnection]:
    if not EXTENSION.is_file():
        pytest.fail(f"no extension at {EXTENSION}; run `make release` or set OXEDI_EXTENSION")
    connection = duckdb.connect(config={"allow_unsigned_extensions": "true"})
    literal = str(EXTENSION).replace("'", "''")
    connection.execute(f"LOAD '{literal}'")
    yield connection
    connection.close()


def digests(rows: Iterable[Sequence[Any]]) -> list[str]:
    """One md5 per row over the repr of each value."""
    return [hashlib.md5(repr(tuple(row)).encode()).hexdigest() for row in rows]


def read_835(
    con: duckdb.DuckDBPyConnection, path: pathlib.Path, table: str, binary: bool
) -> Rows:
    relation = con.execute(
        "SELECT * FROM read_835(?, table_name := ?, binary := ?)", [str(path), table, binary]
    )
    names = [column[0] for column in relation.description]
    return names, [tuple(row) for row in relation.fetchall()]


def oracle_table(result: Any, table: str) -> Rows:
    arrow = pa.table(result.tables[table])
    rows = [tuple(row[name] for name in arrow.column_names) for row in arrow.to_pylist()]
    return list(arrow.column_names), rows


def oracle_diagnostics(result: Any) -> Rows:
    rows = [
        (
            d.level,
            d.kind,
            d.rule,
            d.segment,
            d.element,
            d.component,
            d.path,
            bytes(d.datum),
            d.origin,
            d.code,
        )
        for d in result.diagnostics
    ]
    # datum is the only bytes attribute, as it is the only column that
    # binary := true turns into a BLOB.
    return DIAGNOSTIC_COLUMNS, rows


def assert_same(label: str, ours: Rows, theirs: Rows) -> None:
    assert ours[0] == theirs[0], f"{label}: columns differ"
    mine, oracle = digests(ours[1]), digests(theirs[1])
    assert len(mine) == len(oracle), f"{label}: {len(mine)} rows, oracle {len(oracle)}"
    for index, (a, b) in enumerate(zip(mine, oracle)):
        assert a == b, f"{label}: row {index}: {ours[1][index]!r} != {theirs[1][index]!r}"


def tables_of(result: Any) -> list[str]:
    return sorted(result.tables.keys()) + [DIAGNOSTICS]


def parsed(path: pathlib.Path) -> Any:
    try:
        return oxedi.parse_file(str(path))
    except oxedi.ParseError:
        return None


def as_text(rows: list[tuple[Any, ...]]) -> list[tuple[Any, ...]] | None:
    """The rows with every bytes value decoded; None when one is not UTF-8."""
    try:
        return [
            tuple(value.decode() if isinstance(value, bytes) else value for value in row)
            for row in rows
        ]
    except UnicodeDecodeError:
        return None


PARSABLE = [path for path in FILES if file_id(path) not in UNPARSABLE]


@pytest.mark.parametrize("path", PARSABLE, ids=file_id)
def test_every_table_equals_parse_file(con: duckdb.DuckDBPyConnection, path: pathlib.Path) -> None:
    result = oxedi.parse_file(str(path))
    for table in tables_of(result):
        if table == DIAGNOSTICS:
            expected = oracle_diagnostics(result)
        else:
            expected = oracle_table(result, table)
        label = f"{file_id(path)} {table}"
        assert_same(f"{label} (binary)", read_835(con, path, table, True), expected)
        text = as_text(expected[1])
        if text is None:
            with pytest.raises(duckdb.Error, match=UTF8_ERROR):
                read_835(con, path, table, False)
        else:
            ours = read_835(con, path, table, False)
            assert_same(f"{label} (varchar)", ours, (expected[0], text))


def test_every_sample_and_fixture_is_an_input() -> None:
    folders = [path.parent.name for path in FILES]
    assert folders.count("samples") == 6
    assert folders.count("fixtures") == 5


def test_the_unparsable_inputs_are_pinned() -> None:
    assert {file_id(path) for path in FILES if parsed(path) is None} == set(UNPARSABLE)


@pytest.mark.parametrize(
    "path", [path for path in FILES if file_id(path) in UNPARSABLE], ids=file_id
)
def test_an_unparsable_file(con: duckdb.DuckDBPyConnection, path: pathlib.Path) -> None:
    with pytest.raises(oxedi.ParseError) as raised:
        oxedi.parse_file(str(path))
    message = f'read_835: "{path}" is not an X12 interchange: {raised.value}'
    with pytest.raises(duckdb.Error) as failed:
        con.execute("SELECT * FROM read_835(?)", [str(path)]).fetchall()
    assert message in str(failed.value)
    rows = con.execute(
        "SELECT * FROM read_835(?, table_name := 'diagnostics', ignore_errors := true)",
        [str(path)],
    ).fetchall()
    datum = UNPARSABLE[file_id(path)]
    assert rows == [
        (1, "NotAnInterchange", message, None, None, None, "", datum, "read_835", None)
    ]
    raw = con.execute(
        "SELECT datum FROM read_835(?, table_name := 'diagnostics', ignore_errors := true,"
        " binary := true)",
        [str(path)],
    ).fetchall()
    assert raw == [(datum.encode().decode("unicode_escape").encode("latin-1"),)]
