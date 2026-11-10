"""Reads pyx12's JSON error report into flat findings.

The report holds one node per interchange, group and transaction set, each
with its own errors, and one node per other segment with an error, holding
its segment errors and its element errors.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import Optional, Union

from ._report import ElementNode, Error, Group, Interchange, Report, SegmentNode, Transaction

# Levels follow where pyx12 reports a finding, never its error code: envelope
# findings (interchange, group, transaction) are integrity findings, segment
# and element findings are requirement findings.
ENVELOPE_LEVEL = 1
SEGMENT_LEVEL = 2


class Place(Enum):
    """The segment of its envelope an envelope finding belongs on."""

    HEADER = "header"
    TRAILER = "trailer"


HEADER = Place.HEADER
TRAILER = Place.TRAILER

# Where each interchange (ISA), group (GS) and transaction set (ST) error of
# pyx12 lands, by pyx12's node and error code: on the envelope's header or
# trailer segment, and at the element holding the offending value when one
# does. A trailer finding of an envelope with no trailer stays on its header.
# Codes not listed land on the header with no element.
PLACES: dict[tuple[str, str], tuple[Place, Optional[int]]] = {
    ("ISA", "001"): (TRAILER, 2),  # trailer control number differs from the header's
    ("ISA", "021"): (TRAILER, 1),  # group count is wrong
    ("ISA", "023"): (HEADER, 13),  # trailer missing
    ("ISA", "024"): (TRAILER, None),  # a group inside is not terminated
    ("ISA", "025"): (HEADER, 13),  # control number not unique
    ("GS", "3"): (TRAILER, None),  # a transaction set inside is not terminated, or no trailer
    ("GS", "4"): (TRAILER, 2),  # trailer control number differs from the header's
    ("GS", "5"): (TRAILER, 1),  # transaction set count is wrong
    ("GS", "6"): (HEADER, 6),  # control number not unique
    ("ST", "2"): (HEADER, 2),  # trailer missing
    ("ST", "3"): (TRAILER, 2),  # trailer control number differs from the header's
    ("ST", "4"): (TRAILER, 1),  # segment count is wrong
    ("ST", "23"): (HEADER, 2),  # control number not unique
}

# pyx12 writes one segment error of this name per segment with errors. It
# carries the generic text below when only element errors fired (the element
# findings already carry those), and the text and value of the first folded
# segment-level error (too many elements, a syntax rule) otherwise. The reader
# drops only the generic one, by name and text (a real segment error can share
# its code, 8).
ROLLUP = "SEG_8_has_data_element_errors"
ROLLUP_TEXT = "Segment has data element errors"

# The trailer segment id that closes each envelope.
_TRAILERS = {"ISA": b"IEA", "GS": b"GE", "ST": b"SE"}


@dataclass(frozen=True)
class Envelope:
    """Where in its envelope an envelope finding belongs."""

    header: bytes
    """The id of the envelope's header segment: ``b"ISA"``, ``b"GS"`` or ``b"ST"``."""
    trailer: bytes
    """The id of the trailer segment that closes it: ``b"IEA"``, ``b"GE"`` or ``b"SE"``."""
    place: Place
    """Whether ``PLACES`` puts the finding on the header or on the trailer."""


@dataclass(frozen=True)
class Finding:
    """One error of the report. ``line`` is pyx12's 1-based segment number;
    ``element`` and ``component`` are 1-based positions; ``value`` is what
    pyx12 reports as the offending value."""

    level: int
    code: str
    text: str
    line: Optional[int]
    element: Optional[int] = None
    component: Optional[int] = None
    value: Optional[str] = None
    envelope: Optional[Envelope] = None
    """For an envelope finding, its envelope and the segment of it the
    finding belongs on; ``line`` is then the trailer's line once pyx12 read
    the trailer, the header's otherwise. ``None`` for a segment finding."""


def _text(e: Error) -> str:
    """pyx12's message, followed by its own name for the error unless that is
    the code itself (a legacy code pyx12 passes through)."""
    return e.text if e.name == e.code else f"{e.text} ({e.name})"


_Node = Union[Interchange, Group, Transaction]


def _envelope(kind: str, node: _Node) -> list[Finding]:
    found: list[Finding] = []
    for e in node.errors:
        place, position = PLACES.get((kind, e.code), (HEADER, None))
        envelope = Envelope(kind.encode("ascii"), _TRAILERS[kind], place)
        found.append(
            Finding(ENVELOPE_LEVEL, e.code, _text(e), node.line, element=position, envelope=envelope)
        )
    return found


def _element(seg: SegmentNode, el: ElementNode, e: Error) -> Finding:
    return Finding(
        SEGMENT_LEVEL, e.code, _text(e), seg.line,
        element=el.position, component=el.component, value=e.value,
    )


def _segment(seg: SegmentNode) -> list[Finding]:
    found = [
        Finding(SEGMENT_LEVEL, e.code, _text(e), seg.line, value=e.value)
        for e in seg.errors
        if not (e.name == ROLLUP and e.text == ROLLUP_TEXT)
    ]
    found += [_element(seg, el, e) for el in seg.elements for e in el.errors]
    return found


def collect(report: Report) -> list[Finding]:
    """Every error of ``report``, in document order: an envelope's own
    errors, then those of its children."""
    found: list[Finding] = []
    for isa in report.interchanges:
        found += _envelope("ISA", isa)
        for gs in isa.groups:
            found += _envelope("GS", gs)
            for st in gs.transactions:
                found += _envelope("ST", st)
                found += [f for seg in st.segments for f in _segment(seg)]
    return found
