"""The compat spec, and its tables as Python columns split per transaction."""

from __future__ import annotations

import decimal as decimal_module
import functools
import json
from collections import defaultdict
from typing import Optional, Tuple

from .. import ParseError, Spec, parse
from .._core import EDI_835_PARSER_PATCH
from ._convert import ENCODING, STRIPPED, unpadded

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


def position(table: str, column: str) -> Tuple[int, Optional[int]]:
    """The element and component (``None`` for a whole element) that the patch
    reads for ``column`` of ``table``."""
    source = PATCH["tables"][table]["columns"][column]
    return source["element"], source.get("component")


class Rows:
    """One table's rows of one transaction: an Arrow table, read as columns of
    Python values on demand."""

    def __init__(self, table):
        self.table = table
        self._columns = {}
        self._groups = {}

    @classmethod
    def empty(cls):
        """A table with no rows, whose every column reads as empty."""
        return cls(None)

    def __len__(self):
        return 0 if self.table is None else self.table.num_rows

    def __getitem__(self, column):
        if column not in self._columns:
            self._columns[column] = [] if self.table is None else self.table.column(column).to_pylist()
        return self._columns[column]

    def under(self, parent, key):
        """Positions of the rows whose ``parent`` column is ``key``, in file
        order: the join from one loop level's table to the level it nests in."""
        if parent not in self._groups:
            groups = defaultdict(list)
            for at, value in enumerate(self[parent]):
                groups[value].append(at)
            self._groups[parent] = groups
        return self._groups[parent].get(key, [])


def ints(column):
    """An integer column as a NumPy array, with -1 for null."""
    import pyarrow.compute as pc

    return pc.fill_null(column, -1).to_numpy()


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
    start = int(anchors[row])
    if own is None or document[start].id == own:
        return start
    for index in range(start + 1, bound):
        if document[index].id == own:
            return index
    return None


def _group_last(document, index, source, repeat):
    """The raw text a repeated group's column reads from segment ``index``
    when that is the segment's last element (and component), else ``None``:
    the group whose element ``group_element`` is the last one."""
    elements = document[index].elements
    offset = len(elements) - repeat["from"] - source["group_element"]
    if offset < 0 or offset % repeat["step"]:
        return None
    return _last(document, index, len(elements), source.get("component"))


def _replace(table, column, changes, kind):
    """``table`` with the cells of ``column`` at the positions of ``changes``
    set to their values."""
    import pyarrow as pa

    values = table.column(column).to_pylist()
    for row, value in changes.items():
        values[row] = value
    at = table.schema.get_field_index(column)
    return table.set_column(at, table.schema.field(at), pa.array(values, kind))


@functools.cache
def _endings():
    """The bytes that may end whitespace: the ASCII bytes ``str.strip``
    removes and every non-ASCII byte (the last byte of a multi-byte
    character)."""
    import numpy as np

    table = np.zeros(256, dtype=bool)
    table[list(STRIPPED)] = True
    table[0x80:] = True
    return table


def _padded(view, delimiters):
    """Whether any segment may end in whitespace: a byte that may end
    whitespace right before a segment terminator, or at the end of an input
    whose last segment has no terminator. Always true when a release
    character can escape a terminator."""
    import numpy as np

    if delimiters.release is not None or len(delimiters.segment) != 1:
        return True
    data = np.frombuffer(view, dtype=np.uint8)
    ends = np.flatnonzero(data == delimiters.segment[0])
    if _endings()[data[ends[ends > 0] - 1]].any():
        return True
    tail = data[ends[-1] + 1:] if len(ends) else data
    blank = np.zeros(256, dtype=bool)
    blank[list(STRIPPED)] = True
    return bool(len(tail) and not blank[tail].all() and _endings()[tail[-1]])


def _ending_in_blanks(column):
    """Positions of the binary cells whose last byte may end whitespace."""
    import numpy as np
    import pyarrow as pa
    import pyarrow.compute as pc

    last = pc.binary_slice(column, -1).cast(pa.binary())
    ends = np.zeros(len(column), dtype=np.int64) - 1
    present = ~last.is_null().to_numpy(zero_copy_only=False)
    lengths = pc.fill_null(pc.binary_length(last), 0).to_numpy(zero_copy_only=False)
    rows = np.flatnonzero(present & (lengths > 0))
    if not len(rows):
        return rows
    flat = np.frombuffer(b"".join(last.take(pa.array(rows)).to_pylist()), dtype=np.uint8)
    ends[rows] = flat
    return rows[_endings()[ends[rows]]]


def unpad(document, diagnostics, arrow, padded=True):
    """edi-835-parser strips each segment, so whitespace before the
    terminator never reaches its last element. A binary cell that ends in
    whitespace is stripped when it reads its segment's last element, and a
    decimal cell (a float column here) that the spec could not read because
    of it is read from the stripped text. Only those cells are looked up in
    the document: binary cells whose last byte may end whitespace, and null
    decimal cells in a segment the parser reported (a decimal it could not
    read always is; in a repeated group, only the segment's last group).
    When no segment can end in whitespace (``padded`` false), binary cells
    are not looked at. Returns the tables with those cells replaced."""
    import numpy as np
    import pyarrow as pa

    reported = [d.segment for d in diagnostics if d.segment is not None]
    diagnosed = np.unique(np.array(reported, dtype=np.int64))
    if not padded and not len(diagnosed):
        return arrow
    for name, config in PATCH["tables"].items():
        if config is None or name not in arrow:
            continue
        table = arrow[name]
        if "segment" not in table.column_names:
            continue
        anchors = ints(table.column("segment"))
        ordered = np.sort(anchors[anchors >= 0])
        anchored = config.get("segment")
        repeat = config.get("repeat")
        last_of_segment = np.r_[anchors[1:] != anchors[:-1], True] if len(anchors) else anchors.astype(bool)
        # A row can read a cell the parser flagged only when a flagged
        # segment lies between its anchor and the next row's anchor.
        at = np.searchsorted(ordered, anchors, side="right")
        following = ordered[np.minimum(at, len(ordered) - 1)] if len(ordered) else 0
        bounds = np.where(at < len(ordered), following, len(document))
        flagged = (np.searchsorted(diagnosed, bounds) > np.searchsorted(diagnosed, anchors)) & (anchors >= 0)
        for column, source in config["columns"].items():
            grouped = "group_element" in source
            if source.get("element") is None and not grouped:
                continue
            kind = table.schema.field(column).type
            if pa.types.is_binary(kind):
                if not padded:
                    continue
                candidates = _ending_in_blanks(table.column(column))
            elif pa.types.is_floating(kind):
                nulls = table.column(column).is_null().to_numpy(zero_copy_only=False) & flagged
                candidates = np.flatnonzero(nulls & last_of_segment) if grouped else np.flatnonzero(nulls)
            else:
                continue
            own = None if anchored else source["segment"].encode()
            changes = {}
            for row in candidates:
                if anchors[row] < 0:
                    continue
                if grouped:
                    if not last_of_segment[row]:
                        continue
                    raw = _group_last(document, int(anchors[row]), source, repeat)
                else:
                    at = int(np.searchsorted(ordered, anchors[row], side="right"))
                    bound = int(ordered[at]) if at < len(ordered) else len(document)
                    index = _segment_of(document, anchors, row, own, bound)
                    if index is None:
                        continue
                    raw = _last(document, index, source["element"], source.get("component"))
                if raw is None:
                    continue
                stripped = unpadded(raw)
                if stripped == raw:
                    continue
                if pa.types.is_binary(kind):
                    changes[int(row)] = stripped
                else:
                    try:
                        changes[int(row)] = float(decimal_module.Decimal(stripped.decode(ENCODING)))
                    except (ValueError, decimal_module.InvalidOperation):
                        pass
            if changes:
                table = _replace(table, column, changes, kind)
        arrow[name] = table
    return arrow


def _floats(table):
    """``table`` with its decimal columns as float64, the library's ``float``
    of each value: the decimal's exact text read as the nearest double."""
    import pyarrow as pa

    for at, field in enumerate(table.schema):
        if pa.types.is_decimal(field.type):
            column = table.column(at).cast(pa.string()).cast(pa.float64())
            table = table.set_column(at, pa.field(field.name, pa.float64(), field.nullable), column)
    return table


def _split(table, column, keys):
    """For each key, the slice of ``table`` whose ``column`` equals it; rows
    are in file order, so each transaction's rows are contiguous."""
    import numpy as np
    import pyarrow as pa

    ids = ints(table.column(column))
    if len(ids) and np.any(ids[1:] < ids[:-1]):
        return [table.filter(pa.array(ids == key)) for key in keys]
    starts = np.searchsorted(ids, keys, side="left")
    ends = np.searchsorted(ids, keys, side="right")
    return [table.slice(int(a), int(b - a)) for a, b in zip(starts, ends)]


def load(data):
    """Parses ``data`` (bytes or any buffer) with the compat spec, with the GIL
    released, and returns the document and, per transaction, the tables
    limited to its rows (split in one pass over each table). A table column
    cannot read a value from an enclosing loop, so each loop level is its own
    table, gathered through the parent-row columns (``payment``, ``claim``,
    ``service``).

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
    tables = {name: _floats(pa.table(result.tables[name])) for name in result.tables}
    padded = _padded(view[start:] if bom else view, result.document.delimiters)
    arrow = unpad(result.document, result.diagnostics, tables, padded)
    payments = arrow["rows_payments"]
    interchanges = arrow["rows_interchanges"]
    if not payments.num_rows:
        return result.document, [{
            name: Rows(table if name == "rows_interchanges" and not bom else table.slice(0, 0))
            for name, table in arrow.items()
        }]
    keys = ints(payments.column("row"))
    split = {
        name: _split(table, "row" if name == "rows_payments" else "payment", keys)
        for name, table in arrow.items() if name != "rows_interchanges"
    }
    rows = ints(interchanges.column("row"))
    parts = []
    for at, interchange in enumerate(ints(payments.column("interchange"))):
        part = {name: Rows(tables[at]) for name, tables in split.items()}
        own = interchanges.slice(0, 0) if bom else interchanges.filter(pa.array(rows == interchange))
        part["rows_interchanges"] = Rows(own)
        parts.append(part)
    return result.document, parts
