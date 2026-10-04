"""The compat spec, and its tables as Python columns split per transaction."""

from __future__ import annotations

import functools
import json
from collections import defaultdict

from .. import ParseError, Spec, parse
from .._core import EDI_835_PARSER_PATCH

PATCH = json.loads(EDI_835_PARSER_PATCH)
TABLES = sorted(name for name, table in PATCH["tables"].items() if table is not None)


@functools.cache
def spec() -> Spec:
    """The built-in spec with the edi-835-parser tables in place of the native ones."""
    return Spec.builtin().patch(EDI_835_PARSER_PATCH)


@functools.cache
def interchange_trigger() -> bytes:
    """The segment id that opens an interchange, as the spec declares it."""
    return json.loads(spec().to_json())["loops"]["interchange"]["trigger"]["segment"].encode()


def position(table, column):
    """The element and component (``None`` for a whole element) that the patch
    reads for ``column`` of ``table``."""
    source = PATCH["tables"][table]["columns"][column]
    return source["element"], source.get("component")


class Rows:
    """One table's rows, as columns of Python values."""

    def __init__(self, columns, keep):
        self.columns = {key: [v for v, k in zip(values, keep) if k] for key, values in columns.items()}
        self._groups = {}

    @classmethod
    def empty(cls):
        """A table with no rows, whose every column reads as empty."""
        rows = cls({}, [])
        rows.columns = defaultdict(list)
        return rows

    def __len__(self):
        return len(self.columns["row"])

    def __getitem__(self, column):
        return self.columns[column]

    def under(self, parent, key):
        """Positions of the rows whose ``parent`` column is ``key``, in file order."""
        if parent not in self._groups:
            groups = defaultdict(list)
            for at, value in enumerate(self.columns[parent]):
                groups[value].append(at)
            self._groups[parent] = groups
        return self._groups[parent].get(key, [])


def load(data):
    """Parses ``data`` (bytes or any buffer) with the compat spec, with the GIL
    released, and returns the document and, per transaction, the tables
    limited to its rows.

    The library builds one transaction set from any file, reading whatever
    segments it recognises, so input that does not start with the
    interchange's opening segment gives
    one part with every table empty and no document, and an interchange
    without transactions gives one part with only its interchange rows. Any
    other ``ParseError`` (an ISA that cannot be read) propagates."""
    import pyarrow as pa

    try:
        result = parse(data, spec=spec())
    except ParseError:
        trigger = interchange_trigger()
        if bytes(memoryview(data)[:len(trigger)]) == trigger:
            raise
        return None, [{name: Rows.empty() for name in TABLES}]
    tables = {name: pa.table(result.tables[name]).to_pydict() for name in result.tables}
    payments = tables["rows_payments"]
    if not payments["row"]:
        return result.document, [{
            name: Rows(columns, [name == "rows_interchanges"] * len(columns["row"]))
            for name, columns in tables.items()
        }]
    parts = []
    for payment, interchange in zip(payments["row"], payments["interchange"]):
        part = {}
        for name, columns in tables.items():
            if name == "rows_interchanges":
                keep = [row == interchange for row in columns["row"]]
            elif name == "rows_payments":
                keep = [row == payment for row in columns["row"]]
            else:
                keep = [owner == payment for owner in columns["payment"]]
            part[name] = Rows(columns, keep)
        parts.append(part)
    return result.document, parts
