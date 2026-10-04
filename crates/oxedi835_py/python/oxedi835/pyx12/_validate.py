"""``validate``: run pyx12's validation in-process and translate its findings."""

from __future__ import annotations

import io
import json
import logging
import os
import re
from bisect import bisect_right
from collections import defaultdict, deque
from typing import Any, BinaryIO, Union

from .. import parse
from ._diagnostic import Pyx12Diagnostic

Source = Union[bytes, bytearray, memoryview, str, "os.PathLike[str]", BinaryIO]

_ISA_LENGTH = 106
_NODE_LOG = re.compile(r"^Line:(\d+) (ISA|GS|ST):(\S+) - (.*)$", re.DOTALL)
_OBJECT_REPR = re.compile(r'"<_io\.StringIO object at 0x[0-9a-f]+>"')
_NODE_KIND = {"ISA": "Pyx12InterchangeError", "GS": "Pyx12GroupError", "ST": "Pyx12TransactionError"}


def _import_pyx12() -> Any:
    try:
        import pyx12.params
        import pyx12.x12n_document
    except ImportError as err:
        raise ImportError(
            "oxedi835.pyx12 needs the pyx12 package: pip install 'oxedi835[pyx12]'"
        ) from err
    return pyx12


def _read(source: Source) -> bytes:
    if isinstance(source, (bytes, bytearray, memoryview)):
        return bytes(source)
    if isinstance(source, (str, os.PathLike)):
        with open(source, "rb") as handle:
            return handle.read()
    return source.read()


class _Positions:
    """Maps pyx12's segment numbers to segment indexes and byte ranges of the document.

    pyx12 splits its input on the segment terminator and drops the CR and LF
    that follow it, and counts the pieces from the ISA on; the same split is
    done here to get the offset of each piece, and the document segment whose
    raw bytes hold that offset is the segment pyx12 meant. The document's own
    segments hold their leading trivia, so a byte order mark or blank lines
    before the ISA shift nothing.
    """

    def __init__(self, ends: list[int], base: int, starts: list[int]):
        self.ends = ends
        self.base = base
        self.piece_starts = starts

    @classmethod
    def build(cls, document: Any, base: int, text: str) -> "_Positions":
        ends: list[int] = []
        offset = 0
        for index in range(len(document)):
            offset += len(document[index].raw)
            ends.append(offset)
        terminator = text[_ISA_LENGTH - 1] if len(text) >= _ISA_LENGTH else "~"
        starts: list[int] = []
        position = 0
        while True:
            found = text.find(terminator, position)
            if found == -1:
                break
            piece = text[position:found]
            stripped = piece.lstrip("\n\r")
            if stripped == "":
                break
            starts.append(position + len(piece) - len(stripped))
            position = found + 1
        return cls(ends, base, starts)

    def locate(self, line: int | None) -> tuple[int | None, tuple[int, int] | None]:
        """The segment index and byte range of pyx12's 1-based segment ``line``."""
        if line is None or not 1 <= line <= len(self.piece_starts):
            return None, None
        offset = self.base + self.piece_starts[line - 1]
        index = bisect_right(self.ends, offset)
        if index >= len(self.ends):
            return None, None
        start = self.ends[index - 1] if index else 0
        return index, (start, self.ends[index])


class _LogCapture(logging.Handler):
    """Collects what pyx12 logs at error level while it validates."""

    def __init__(self) -> None:
        super().__init__(logging.ERROR)
        self.messages: list[str] = []

    def emit(self, record: logging.LogRecord) -> None:
        self.messages.append(record.getMessage())


def _run(text: str, track: list[int]) -> tuple[bool, Any, list[str], BaseException | None]:
    import pyx12.params
    import pyx12.x12n_document

    def callback(_seg: Any, src: Any, _node: Any, _valid: bool) -> None:
        track.append(src.get_cur_line())

    capture = _LogCapture()
    logger = logging.getLogger("pyx12")
    previous = logger.propagate
    logger.addHandler(capture)
    logger.propagate = False
    errors = io.StringIO()
    failure: BaseException | None = None
    ok = False
    try:
        ok = pyx12.x12n_document.x12n_document(
            param=pyx12.params.params(),
            src_file=io.StringIO(text),
            fd_997=None,
            fd_html=None,
            fd_json=errors,
            callback=callback,
        )
    except Exception as err:  # pyx12 is third-party: any failure is a finding
        failure = err
    finally:
        logger.removeHandler(capture)
        logger.propagate = previous
    tree = None
    if failure is None and errors.getvalue():
        try:
            tree = json.loads(errors.getvalue())
        except ValueError as err:
            failure = err
    return ok, tree, capture.messages, failure


def _datum(value: Any) -> bytes:
    return b"" if value is None else str(value).encode("latin-1", "replace")


def _node_lines(messages: list[str]) -> dict[tuple[str, str, str], deque[int]]:
    lines: dict[tuple[str, str, str], deque[int]] = defaultdict(deque)
    for message in messages:
        match = _NODE_LOG.match(message)
        if match:
            lines[(match[2], match[3], match[4])].append(int(match[1]))
    return lines


def _translate(tree: dict[str, Any], messages: list[str], positions: _Positions):
    lines = _node_lines(messages)
    out: list[Pyx12Diagnostic] = []

    def node(scope: str, entry: dict[str, Any]) -> None:
        for error in entry["errors"]:
            code, text = error["err_cde"], error["err_str"]
            pending = lines.get((scope, code, text))
            line = pending.popleft() if pending else entry["cur_line"]
            segment, span = positions.locate(line)
            out.append(
                Pyx12Diagnostic(
                    kind=_NODE_KIND[scope], rule=text, code=code, segment=segment, span=span
                )
            )

    for isa in tree["interchanges"]:
        node("ISA", isa)
        for group in isa["groups"]:
            node("GS", group)
            for transaction in group["transactions"]:
                node("ST", transaction)
                for seg in transaction["segments"]:
                    segment, span = positions.locate(seg["cur_line"])
                    for error in seg["errors"]:
                        out.append(
                            Pyx12Diagnostic(
                                kind="Pyx12SegmentError",
                                rule=error["err_str"],
                                code=error["err_cde"],
                                segment=segment,
                                span=span,
                                segment_name=seg["name"],
                                datum=_datum(error["err_val"]),
                            )
                        )
                    for element in seg["elements"]:
                        for error in element["errors"]:
                            out.append(
                                Pyx12Diagnostic(
                                    kind="Pyx12ElementError",
                                    rule=error["err_str"],
                                    code=error["err_cde"],
                                    segment=segment,
                                    span=span,
                                    element=element["ele_pos"],
                                    component=element["subele_pos"],
                                    segment_name=seg["name"],
                                    datum=_datum(error["err_val"]),
                                )
                            )
    return out


def _failure(
    reason: str, track: list[int], positions: _Positions, document: Any
) -> Pyx12Diagnostic:
    segment, span = positions.locate(track[-1] if track else None)
    datum = bytes(document[segment].id) if segment is not None else b""
    where = (
        f"the last segment it reached is #{segment}"
        if segment is not None
        else "it reached no segment"
    )
    return Pyx12Diagnostic(
        kind="Pyx12Failure",
        rule=f"pyx12 could not finish validating: {reason}; {where}",
        segment=segment,
        span=span,
        datum=datum,
    )


def validate(source: Source) -> list[Pyx12Diagnostic]:
    """Validates an 835 file with pyx12 and returns what it finds.

    ``source`` is the file's bytes, a path, or a file object open in binary
    mode. pyx12 picks its map from the file's own version declaration. A
    finding is a :class:`Pyx12Diagnostic`; a file pyx12 cannot read, or an
    exception inside pyx12, gives one ``Pyx12Failure`` diagnostic instead of
    raising. A file with no ISA to read the delimiters from raises
    ``oxedi835.ParseError``, as ``oxedi835.parse`` does. Nothing is written to
    disk and no acknowledgement is generated.
    """
    _import_pyx12()
    data = _read(source)
    document = parse(data).document
    base = data.find(b"ISA")
    text = data[max(base, 0):].decode("latin-1")
    positions = _Positions.build(document, max(base, 0), text)
    track: list[int] = []
    ok, tree, messages, failure = _run(text, track)
    if failure is not None:
        return [_failure(f"{type(failure).__name__}: {failure}", track, positions, document)]
    if tree is None:
        detail = _OBJECT_REPR.sub("the input", messages[-1]) if messages else "no reason given"
        return [_failure(detail, track, positions, document)] if not ok else []
    return _translate(tree, messages, positions)
