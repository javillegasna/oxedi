"""The compat spec, and its tables as Python columns split per transaction."""

from __future__ import annotations

import bisect
import decimal as decimal_module
import functools
import json
from collections import defaultdict

from .. import ParseError, Spec, parse
from .._core import EDI_835_PARSER_PATCH
from ._convert import STRIPPED

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
        """Positions of the rows whose ``parent`` column is ``key``, in file
        order: the join from one loop level's table to the level it nests in."""
        if parent not in self._groups:
            groups = defaultdict(list)
            for at, value in enumerate(self.columns[parent]):
                groups[value].append(at)
            self._groups[parent] = groups
        return self._groups[parent].get(key, [])


BOM = b"\xef\xbb\xbf"


def opening(view, trigger):
    """The offset of ``trigger`` when ``view`` is a UTF-8 byte order mark, then
    optional whitespace (any ASCII byte ``str.strip`` removes), then
    ``trigger``; otherwise ``None``. Parsing starts at that offset, so the
    parser never sees the mark or the whitespace."""
    if bytes(view[:len(BOM)]) != BOM:
        return None
    at = len(BOM)
    while at < len(view) and view[at] in STRIPPED:
        at += 1
    return at if bytes(view[at:at + len(trigger)]) == trigger else None


def _last(document, index, element, component):
    """The raw text of ``element`` (and ``component``) of segment ``index``
    when it is the segment's last one, else ``None``."""
    elements = document[index].elements
    if element != len(elements):
        return None
    value = elements[element - 1]
    if component is None:
        return value if isinstance(value, bytes) else None
    if isinstance(value, list):
        return value[component - 1] if component == len(value) else None
    return value if component == 1 else None


def _segment_of(document, anchors, row, own, bound):
    """The index of the segment a column reads for ``row``: the anchor
    segment, or the first segment named ``own`` after it and before the next
    row's anchor."""
    start = anchors[row]
    if own is None or document[start].id == own:
        return start
    for index in range(start + 1, bound):
        if document[index].id == own:
            return index
    return None


def unpad(document, arrow, tables):
    """edi-835-parser strips each segment, so blanks before the terminator
    never reach its last element. A binary cell that ends in blanks is
    stripped when it is its segment's last element, and a decimal cell that
    the spec could not read because of them is read from the stripped text.
    Only cells that end in blanks, or decimal cells that are null, are
    looked up in the document."""
    import pyarrow as pa

    for name, columns in PATCH["tables"].items():
        if columns is None or name not in tables:
            continue
        rows = tables[name]
        anchors = rows.get("segment")
        if anchors is None:
            continue
        anchored = columns.get("segment")
        ordered = sorted(a for a in anchors if a is not None)
        for column, source in columns["columns"].items():
            element = source.get("element")
            if element is None or "group_element" in source:
                continue
            kind = arrow[name].schema.field(column).type
            decimal = pa.types.is_decimal(kind)
            values = rows[column]
            own = None if anchored else source["segment"].encode()
            for row, value in enumerate(values):
                if value is None:
                    if not decimal:
                        continue
                elif not isinstance(value, bytes) or value == value.rstrip(STRIPPED):
                    continue
                if anchors[row] is None:
                    continue
                at = bisect.bisect_right(ordered, anchors[row])
                bound = ordered[at] if at < len(ordered) else len(document)
                index = _segment_of(document, anchors, row, own, bound)
                if index is None:
                    continue
                raw = _last(document, index, element, source.get("component"))
                if raw is None:
                    continue
                stripped = raw.rstrip(STRIPPED)
                if value is None:
                    if stripped != raw:
                        try:
                            values[row] = decimal_module.Decimal(stripped.decode())
                        except (ValueError, decimal_module.InvalidOperation):
                            pass
                else:
                    values[row] = stripped


def load(data):
    """Parses ``data`` (bytes or any buffer) with the compat spec, with the GIL
    released, and returns the document and, per transaction, the tables
    limited to its rows. A table column cannot read a value from an enclosing
    loop, so each loop level is its own table, gathered through the parent-row
    columns (``payment``, ``claim``, ``service``).

    The library builds one transaction set from any file, reading whatever
    segments it recognises:
    - input that does not start with the interchange's opening segment gives
      one part with every table empty and no document;
    - a UTF-8 byte order mark before that segment (with or without
      whitespace between them) stays glued to the first segment in the
      library, which then misses only the interchange, so the input is parsed
      from that segment on and the interchange rows are left out; if that
      parse fails, the result is the part with every table empty, since the
      library never reads the segment that failed;
    - an interchange without transactions gives one part with only its
      interchange rows.
    Any other ``ParseError`` (an opening segment that cannot be read, with
    no mark before it) propagates."""
    import pyarrow as pa

    view = memoryview(data)
    trigger = interchange_trigger()
    start = opening(view, trigger)
    bom = start is not None
    try:
        result = parse(view[start:] if bom else data, spec=spec())
    except ParseError:
        if not bom and bytes(view[:len(trigger)]) == trigger:
            raise
        return None, [{name: Rows.empty() for name in TABLES}]
    arrow = {name: pa.table(result.tables[name]) for name in result.tables}
    tables = {name: table.to_pydict() for name, table in arrow.items()}
    unpad(result.document, arrow, tables)
    payments = tables["rows_payments"]
    if not payments["row"]:
        return result.document, [{
            name: Rows(columns, [name == "rows_interchanges" and not bom] * len(columns["row"]))
            for name, columns in tables.items()
        }]
    parts = []
    for payment, interchange in zip(payments["row"], payments["interchange"]):
        part = {}
        for name, columns in tables.items():
            if name == "rows_interchanges":
                keep = [row == interchange and not bom for row in columns["row"]]
            elif name == "rows_payments":
                keep = [row == payment for row in columns["row"]]
            else:
                keep = [owner == payment for owner in columns["payment"]]
            part[name] = Rows(columns, keep)
        parts.append(part)
    return result.document, parts
