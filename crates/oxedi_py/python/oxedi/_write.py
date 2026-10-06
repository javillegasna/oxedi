"""``write``: tables back into an interchange, and the error that refuses it."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any, Literal, Optional, Union, overload

from ._core import Envelope, Spec, Tables, WriteFinding, _write


class WriteError(ValueError):
    """Tables that cannot be written as a valid file; nothing was written.

    The message names every reason: the rule, where (table, row and column,
    or envelope field) and the value. ``findings`` holds them one by one; it
    is empty when the refusal is about the spec, the tables' columns or the
    delimiters rather than about the data.
    """

    findings: list[WriteFinding]

    def __init__(self, message: str, findings: Optional[list[WriteFinding]] = None) -> None:
        super().__init__(message)
        self.findings = list(findings or [])


TablesLike = Union[Tables, Mapping[str, Any]]


@overload
def write(
    tables: TablesLike,
    envelope: Envelope,
    spec: Optional[Spec] = None,
    allow_findings: Literal[False] = False,
) -> bytes: ...


@overload
def write(
    tables: TablesLike,
    envelope: Envelope,
    spec: Optional[Spec] = None,
    *,
    allow_findings: Literal[True],
) -> tuple[bytes, list[WriteFinding]]: ...


@overload
def write(
    tables: TablesLike,
    envelope: Envelope,
    spec: Optional[Spec] = None,
    allow_findings: bool = False,
) -> Union[bytes, tuple[bytes, list[WriteFinding]]]: ...


def write(
    tables: TablesLike,
    envelope: Envelope,
    spec: Optional[Spec] = None,
    allow_findings: bool = False,
) -> Union[bytes, tuple[bytes, list[WriteFinding]]]:
    """Writes tables with a spec's schema as one interchange.

    ``tables`` is the ``tables`` of a parse, or a mapping of table names to
    Arrow, Polars or pandas tables with the spec's columns (a table left out
    has no rows, a column left out is null). ``spec`` defaults to the spec
    that parsed the tables, else the built-in 5010 spec. The written file is
    read back with the spec, and every diagnostic of that read is a finding.

    A null cell in the middle of a segment is written as an empty element,
    which reads back as empty text; only trailing nulls read back as null.

    Strict by default: any finding raises ``WriteError`` and nothing is
    returned. With ``allow_findings`` the bytes are returned with the
    findings, ``(bytes, findings)``. When a value holds a delimiter the file
    is not read back, so the findings then lack those of reading it.
    """
    data, findings = _write(tables, envelope, spec=spec, allow_findings=allow_findings)
    if allow_findings:
        return data, findings
    return data
