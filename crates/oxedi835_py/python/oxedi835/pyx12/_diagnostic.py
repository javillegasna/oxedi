"""The diagnostic a pyx12 finding becomes."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Optional

ORIGIN = "pyx12"


def _quoted(datum: bytes) -> str:
    text = datum.decode("latin-1").replace("\\", "\\\\").replace('"', '\\"')
    return f'"{text}"'


@dataclass(frozen=True)
class Pyx12Diagnostic:
    """One finding of pyx12, with the attributes of ``oxedi835.Diagnostic``.

    ``level`` is always ``None``: pyx12 findings carry no SNIP level, they carry
    ``code``, pyx12's own error code. ``segment`` is the index of the segment at
    fault in the file's document and ``span`` its byte range, ``(start, end)``,
    both ``None`` when the finding names no segment. ``element`` and
    ``component`` are 1-based. ``path`` is empty: pyx12 does not report the
    loop the segment sits in; ``segment_name`` is the name pyx12's map gives
    the segment.
    """

    kind: str
    rule: str
    code: Optional[str] = None
    segment: Optional[int] = None
    span: Optional[tuple[int, int]] = None
    element: Optional[int] = None
    component: Optional[int] = None
    segment_name: Optional[str] = None
    datum: bytes = b""
    origin: str = ORIGIN
    level: Optional[int] = None
    path: str = ""

    def __str__(self) -> str:
        parts = [f"{self.origin} · {self.rule}"]
        if self.code is not None:
            parts[0] += f" (code {self.code})"
        place = "no segment" if self.segment is None else f"segment #{self.segment}"
        if self.span is not None:
            place += f", bytes {self.span[0]}..{self.span[1]}"
        if self.element is not None:
            place += f", element {self.element}"
            if self.component is not None:
                place += f", component {self.component}"
        parts.append(place)
        parts.append(f"datum {_quoted(self.datum)}")
        return " · ".join(parts)
