"""Lossless, fast, data-driven EDI 835 parser.

``parse`` reads a whole file into a document, typed tables and diagnostics.
``stream`` yields the tables in batches, one per closed loop instance.
Tables export to Arrow through the PyCapsule interface, so Polars, pyarrow
or DuckDB read them without copying.
"""

from __future__ import annotations

import os
from typing import Optional, Union

from ._core import (
    Delimiters,
    Document,
    ParseError,
    Result,
    Segment,
    Spec,
    SpecError,
    parse,
)

__all__ = [
    "Delimiters",
    "Document",
    "ParseError",
    "Result",
    "Segment",
    "Spec",
    "SpecError",
    "parse",
    "parse_file",
]


def parse_file(
    path: Union[str, "os.PathLike[str]"],
    delimiters: Optional[Delimiters] = None,
) -> Result:
    """Reads the file at ``path`` in binary mode and parses it."""
    with open(path, "rb") as handle:
        data = handle.read()
    return parse(data, delimiters=delimiters)
