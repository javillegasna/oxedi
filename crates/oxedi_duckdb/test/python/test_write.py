"""COPY ... (FORMAT edi835) against oxedi.write.

Every sample and fixture is parsed with each built-in version, and its tables are
written twice: by `oxedi.write` and by `COPY` from `read_835`'s tables with the
same envelope. A file `oxedi.write` accepts must come out byte for byte equal; a
file it refuses must fail the `COPY` with the whole text of Python's error, so
nothing is written.

The counts are pinned: a pair that silently stops being accepted or refused
fails the run.
"""

from __future__ import annotations

import datetime
import pathlib
from typing import Any

import duckdb
import oxedi
import pytest

from test_oracle import FILES, UNPARSABLE, con, file_id  # noqa: F401  (con is a fixture)

VERSIONS = ["5010", "4010"]
PARSABLE = [path for path in FILES if file_id(path) not in UNPARSABLE]
# Only the samples and the fixtures: the extension's own data is not 835.
INPUTS = [path for path in PARSABLE if path.parent.name in ("samples", "fixtures")]

# The envelope as Python takes it and as COPY options spell it.
ENVELOPES = {
    "default": (
        {},
        "",
    ),
    "non_default": (
        {"control_number": 7, "line_break": True, "usage_indicator": "T"},
        ", control_number 7, line_break true, usage_indicator 'T'",
    ),
}
ACCEPTED = 8
REFUSED = 16

# The (file, version) pairs by outcome, filled in the first time they are needed.
_outcomes: dict[str, list[tuple[pathlib.Path, str]]] = {}


def envelope(extra: dict[str, Any]) -> Any:
    return oxedi.Envelope(
        sender_id="SENDER",
        receiver_id="RECEIVER",
        date=datetime.date(2024, 1, 2),
        time=datetime.time(10, 30),
        **extra,
    )


def python_write(path: pathlib.Path, version: str, extra: dict[str, Any]) -> bytes:
    result = oxedi.parse_file(str(path), spec=oxedi.Spec.builtin(version))
    return oxedi.write(result.tables, envelope(extra))


def copy_statement(path: pathlib.Path, version: str, options: str, target: pathlib.Path) -> str:
    tables = oxedi.parse_file(str(path), spec=oxedi.Spec.builtin(version)).tables.keys()
    fields = ", ".join(
        f"'{name}': (SELECT list(t ORDER BY t.\"row\") "
        f"FROM read_835('{path}', table_name := '{name}', version := '{version}') t)"
        for name in tables
    )
    return (
        f"COPY (SELECT {{{fields}}}) TO '{target}' (FORMAT edi835, sender_id 'SENDER', "
        f"receiver_id 'RECEIVER', date '2024-01-02', time '10:30', version '{version}'{options})"
    )


def outcomes() -> dict[str, list[tuple[pathlib.Path, str]]]:
    if not _outcomes:
        _outcomes["accepted"], _outcomes["refused"] = [], []
        for path in INPUTS:
            for version in VERSIONS:
                try:
                    python_write(path, version, {})
                    _outcomes["accepted"].append((path, version))
                except oxedi.WriteError:
                    _outcomes["refused"].append((path, version))
    return _outcomes


def pair_id(pair: tuple[pathlib.Path, str]) -> str:
    return f"{file_id(pair[0])}@{pair[1]}"


def test_the_counts_are_pinned() -> None:
    found = outcomes()
    assert len(INPUTS) == 12
    assert len(found["accepted"]) == ACCEPTED
    assert len(found["refused"]) == REFUSED


@pytest.mark.parametrize("name", list(ENVELOPES))
@pytest.mark.parametrize("pair", [(p, v) for p in INPUTS for v in VERSIONS], ids=pair_id)
def test_copy_equals_oxedi_write(
    con: duckdb.DuckDBPyConnection, pair: tuple[pathlib.Path, str], name: str, tmp_path: pathlib.Path
) -> None:
    path, version = pair
    extra, options = ENVELOPES[name]
    target = tmp_path / "out.835"
    statement = copy_statement(path, version, options, target)
    try:
        expected = python_write(path, version, extra)
    except oxedi.WriteError as refused:
        with pytest.raises(duckdb.Error) as failed:
            con.execute(statement)
        assert str(refused) in str(failed.value)
        assert not target.exists()
        return
    assert pair in outcomes()["accepted"]
    con.execute(statement)
    assert target.read_bytes() == expected


def test_a_refused_pair_never_becomes_accepted_with_the_other_envelope() -> None:
    refused = outcomes()["refused"]
    for path, version in refused:
        with pytest.raises(oxedi.WriteError):
            python_write(path, version, ENVELOPES["non_default"][0])
