"""Reads pyx12's error tree into flat findings.

pyx12's error handler holds one node per interchange, group and transaction
set, each with its own errors and the element errors of its header and
trailer segments, and one node per other segment with an error, holding its
segment errors and element errors. A visitor walks it with the handler's
``accept``; the element errors of the envelope nodes, which ``accept``
does not visit, are read from the nodes themselves.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Optional

# Levels follow where pyx12 reports a finding, never its error code: envelope
# findings (interchange, group, transaction) are integrity findings, segment
# and element findings are requirement findings.
ENVELOPE_LEVEL = 1
SEGMENT_LEVEL = 2

HEADER = "header"
TRAILER = "trailer"

# Where each interchange (ISA), group (GS) and transaction set (ST) error of
# pyx12 lands, by pyx12's node and error code: on the envelope's header or
# trailer segment, and at the element holding the offending value when one
# does. A trailer finding of an envelope with no trailer stays on its header.
# Codes not listed land on the header with no element.
PLACES: dict[tuple[str, str], tuple[str, Optional[int]]] = {
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

# The header and trailer line attributes of each envelope node.
_LINES = {
    "ISA": ("cur_line_isa", "cur_line_iea"),
    "GS": ("cur_line_gs", "cur_line_ge"),
    "ST": ("cur_line_st", "cur_line_se"),
}


@dataclass(frozen=True)
class Finding:
    """One error of the tree. ``line`` is pyx12's 1-based segment number;
    ``element`` and ``component`` are 1-based positions; ``value`` is what
    pyx12 reports as the offending value."""

    level: int
    code: str
    text: str
    line: Optional[int]
    element: Optional[int] = None
    component: Optional[int] = None
    value: Optional[str] = None


class _Visitor:
    """Collects the findings of the tree in the order ``accept`` visits it.

    ``element_lines`` holds, by node identity, the segment number at which
    each element error of an envelope segment was raised: the header's or
    the trailer's."""

    def __init__(self, element_lines: dict[int, int]) -> None:
        self.found: list[Finding] = []
        self.element_lines = element_lines

    def _envelope(self, node: Any) -> None:
        header_attr, trailer_attr = _LINES[node.id]
        header = getattr(node, header_attr)
        trailer = getattr(node, trailer_attr)
        for code, text in node.errors:
            place, position = PLACES.get((node.id, code), (HEADER, None))
            if place == TRAILER and not trailer:
                line, position = header, None
            else:
                line = trailer if place == TRAILER else header
            self.found.append(Finding(ENVELOPE_LEVEL, code, text, line, element=position))
        for element in node.elements:
            line = self.element_lines.get(id(element), header)
            self._element(element, line)

    def _element(self, element: Any, line: Optional[int]) -> None:
        for code, text, value in element.errors:
            self.found.append(
                Finding(
                    SEGMENT_LEVEL,
                    code,
                    text,
                    line,
                    element=element.ele_pos,
                    component=element.subele_pos,
                    value=value,
                )
            )

    def visit_root_pre(self, _errh: Any) -> None:
        pass

    def visit_root_post(self, _errh: Any) -> None:
        pass

    def visit_isa_pre(self, node: Any) -> None:
        self._envelope(node)

    def visit_isa_post(self, _node: Any) -> None:
        pass

    def visit_gs_pre(self, node: Any) -> None:
        self._envelope(node)

    def visit_gs_post(self, _node: Any) -> None:
        pass

    def visit_st_pre(self, node: Any) -> None:
        self._envelope(node)

    def visit_st_post(self, _node: Any) -> None:
        pass

    def visit_seg(self, node: Any) -> None:
        for code, text, value in node.errors:
            self.found.append(Finding(SEGMENT_LEVEL, code, text, node.cur_line, value=value))

    def visit_ele(self, node: Any) -> None:
        self._element(node, node.parent.cur_line)


def collect(handler: Any) -> list[Finding]:
    """Every error of ``handler``'s tree, and the segment errors pyx12
    could not attach to it."""
    visitor = _Visitor(handler.oxedi_element_lines)
    handler.accept(visitor)
    for dropped in handler.oxedi_dropped:
        visitor.found.append(
            Finding(SEGMENT_LEVEL, dropped.code, dropped.text, dropped.line, value=dropped.value)
        )
    return visitor.found
