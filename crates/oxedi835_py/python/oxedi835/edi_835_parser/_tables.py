"""The compat spec, and its tables as Python columns split per transaction."""

from __future__ import annotations

import functools
from collections import defaultdict

from .. import Spec, parse
from .._core import EDI_835_PARSER_PATCH


@functools.cache
def spec() -> Spec:
    """The built-in spec with the edi-835-parser tables in place of the native ones."""
    return Spec.builtin().patch(EDI_835_PARSER_PATCH)


class Rows:
    """One table's rows, as columns of Python values."""

    def __init__(self, columns, keep):
        self.columns = {key: [v for v, k in zip(values, keep) if k] for key, values in columns.items()}
        self._groups = {}

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
    limited to its rows."""
    import pyarrow as pa

    result = parse(data, spec=spec())
    tables = {name: pa.table(result.tables[name]).to_pydict() for name in result.tables}
    payments = tables["rows_payments"]
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
