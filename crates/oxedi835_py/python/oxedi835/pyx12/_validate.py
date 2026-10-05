"""``validate``: run pyx12's validation in-process and translate its findings."""

from __future__ import annotations

import contextlib
import io
import json
import logging
import os
import re
import threading
from bisect import bisect_right
from collections import defaultdict, deque
from typing import Any, BinaryIO, Union

from .. import Diagnostic, parse
from .._core import _external_diagnostic

Source = Union[bytes, bytearray, memoryview, str, "os.PathLike[str]", BinaryIO]

_ISA_LENGTH = 106
_NODE_LOG = re.compile(r"^Line:(\d+) (ISA|GS|ST):(\S+) - (.*)$", re.DOTALL)
_OBJECT_REPR = re.compile(r'"<_io\.StringIO object at 0x[0-9a-f]+>"')
_ORIGIN = "pyx12"
# Levels follow where pyx12 reports a finding, never its error code: envelope
# findings (interchange, group, transaction) and failures are integrity
# findings, segment and element findings are requirement findings.
_ENVELOPE_LEVEL = 1
_SEGMENT_LEVEL = 2
_FAILURE_LEVEL = 1


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
    """Maps pyx12's segment numbers to segment indexes of the document.

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

    def locate(self, line: int | None) -> int | None:
        """The segment index of pyx12's 1-based segment ``line``."""
        if line is None or not 1 <= line <= len(self.piece_starts):
            return None
        offset = self.base + self.piece_starts[line - 1]
        index = bisect_right(self.ends, offset)
        return index if index < len(self.ends) else None


class _LogCapture(logging.Handler):
    """Collects what pyx12 logs at error level while it validates."""

    def __init__(self) -> None:
        super().__init__(logging.ERROR)
        self.messages: list[str] = []

    def emit(self, record: logging.LogRecord) -> None:
        self.messages.append(record.getMessage())


_LOCK = threading.Lock()


def _pyx12_loggers() -> list[logging.Logger]:
    names = [n for n in list(logging.root.manager.loggerDict) if n == "pyx12" or n.startswith("pyx12.")]
    return [logging.getLogger(n) for n in names if isinstance(logging.root.manager.loggerDict[n], logging.Logger)]


@contextlib.contextmanager
def _isolated_logging(capture: logging.Handler):
    """Routes pyx12's error records to ``capture`` whatever the caller's logging
    configuration, and restores that configuration on exit. Callers hold ``_LOCK``."""
    root = logging.getLogger("pyx12")
    loggers = _pyx12_loggers()
    saved = [(lg, lg.level, lg.disabled) for lg in loggers]
    propagate = root.propagate
    disabled_below = logging.root.manager.disable
    # process-wide for the duration of the call (a global switch); restored on exit
    logging.disable(logging.NOTSET)
    for lg in loggers:
        lg.disabled = False
        lg.setLevel(logging.NOTSET)
    root.setLevel(logging.ERROR)
    root.propagate = False
    root.addHandler(capture)
    try:
        yield
    finally:
        root.removeHandler(capture)
        root.propagate = propagate
        for lg, level, disabled in saved:
            lg.setLevel(level)
            lg.disabled = disabled
        logging.disable(disabled_below)


def _run(text: str, track: list[int]) -> tuple[bool, Any, list[str], BaseException | None]:
    import pyx12.params
    import pyx12.x12n_document

    def callback(_seg: Any, src: Any, _node: Any, _valid: bool) -> None:
        track.append(src.get_cur_line())

    capture = _LogCapture()
    errors = io.StringIO()
    failure: BaseException | None = None
    ok = False
    with _LOCK, _isolated_logging(capture):
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


def _finding(message: str, level: int, **place: Any) -> Diagnostic:
    return _external_diagnostic(_ORIGIN, message, level, **place)


def _translate(
    tree: dict[str, Any], messages: list[str], positions: _Positions
) -> list[Diagnostic]:
    lines = _node_lines(messages)
    out: list[Diagnostic] = []

    def node(scope: str, entry: dict[str, Any]) -> None:
        for error in entry["errors"]:
            code, text = error["err_cde"], error["err_str"]
            pending = lines.get((scope, code, text))
            line = pending.popleft() if pending else entry["cur_line"]
            out.append(
                _finding(text, _ENVELOPE_LEVEL, code=code, segment=positions.locate(line))
            )

    for isa in tree["interchanges"]:
        node("ISA", isa)
        for group in isa["groups"]:
            node("GS", group)
            for transaction in group["transactions"]:
                node("ST", transaction)
                for seg in transaction["segments"]:
                    segment = positions.locate(seg["cur_line"])
                    for error in seg["errors"]:
                        out.append(
                            _finding(
                                error["err_str"],
                                _SEGMENT_LEVEL,
                                code=error["err_cde"],
                                segment=segment,
                                datum=_datum(error["err_val"]),
                            )
                        )
                    for element in seg["elements"]:
                        for error in element["errors"]:
                            out.append(
                                _finding(
                                    error["err_str"],
                                    _SEGMENT_LEVEL,
                                    code=error["err_cde"],
                                    segment=segment,
                                    element=element["ele_pos"],
                                    component=element["subele_pos"],
                                    datum=_datum(error["err_val"]),
                                )
                            )
    return out


def _failure(
    reason: str, track: list[int], positions: _Positions, document: Any, started: bool = True
) -> Diagnostic:
    """One failure finding, with no code: pyx12 did not report it, it stopped.

    ``track`` holds the segment numbers pyx12 finished; the segment it was
    working on when it failed is the one after the last."""
    current = (track[-1] + 1 if track else 1) if started else None
    segment = positions.locate(current)
    if segment is None:
        where = "it reached no segment"
        datum = b""
    else:
        datum = bytes(document[segment].id)
        done = positions.locate(track[-1]) if track else None
        where = f"it was processing segment #{segment}" + (
            f"; the last it completed is #{done}" if done is not None else ""
        )
    return _finding(
        f"could not finish validating: {reason}; {where}",
        _FAILURE_LEVEL,
        segment=segment,
        datum=datum,
    )


def _rejection(text: str, fallback: str) -> str:
    """The reason pyx12 refuses to read ``text`` as X12, which it logs without the
    exception text."""
    import pyx12.errors
    import pyx12.rawx12file

    try:
        pyx12.rawx12file.RawX12File(io.StringIO(text))
    except pyx12.errors.X12Error as err:
        return f"X12Error: {str(err).strip()}"
    return fallback


def validate(source: Source) -> list[Diagnostic]:
    """Validates an 835 file with pyx12 and returns what it finds.

    ``source`` is the file's bytes, a path, or a file object open in binary
    mode. pyx12 picks its map from the file's own version declaration.

    Each finding is an :class:`oxedi835.Diagnostic`, the type ``parse``
    returns, so both lists mix, sort by ``level`` and filter by ``origin``.
    A pyx12 finding has ``kind == "External"``, ``origin == "pyx12"`` and
    ``code`` set to pyx12's own error code; ``rule`` is pyx12's message.
    ``level`` follows where pyx12 reports it: interchange, group and
    transaction findings are level 1, segment and element findings level 2.
    ``segment`` is the index of the segment at fault in the file's document
    (``None`` when pyx12 names none), and ``document[d.segment].span`` gives
    its byte range. ``path`` is empty: pyx12 does not report the loop.

    A file pyx12 cannot read, an exception inside pyx12, or a report that
    cannot be translated gives one level 1 finding with no ``code`` whose
    ``rule`` starts with ``could not finish validating``, instead of
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
        if ok:
            return []
        detail = _OBJECT_REPR.sub("the input", messages[-1]) if messages else "no reason given"
        return [_failure(_rejection(text, detail), track, positions, document, started=False)]
    try:
        return _translate(tree, messages, positions)
    except Exception as err:  # pyx12's JSON shape is not ours: report it, do not raise
        reason = f"its report could not be translated ({type(err).__name__}: {err})"
        return [_failure(reason, track, positions, document, started=False)]
