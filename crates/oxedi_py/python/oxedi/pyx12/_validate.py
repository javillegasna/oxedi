"""``validate``: run pyx12's validation in-process and translate its error tree."""

from __future__ import annotations

import io
import os
from bisect import bisect_right
from typing import Any, BinaryIO, Optional, Union

from .. import Diagnostic, parse
from .._core import _external_diagnostic
from . import _capture
from ._tree import Finding, collect

Source = Union[bytes, bytearray, memoryview, str, "os.PathLike[str]", BinaryIO]

_ISA_LENGTH = 106
_ORIGIN = "pyx12"
_FAILURE_LEVEL = 1


def _import_pyx12() -> Any:
    try:
        import pyx12.error_handler
        import pyx12.params
        import pyx12.x12n_document
    except ImportError as err:
        raise ImportError(
            "oxedi.pyx12 needs the pyx12 package: pip install 'oxedi[pyx12]'"
        ) from err
    _capture.install(pyx12.error_handler)
    return pyx12


def _read(source: Source) -> bytes:
    if isinstance(source, (bytes, bytearray, memoryview)):
        return bytes(source)
    if isinstance(source, (str, os.PathLike)):
        with open(source, "rb") as handle:
            return handle.read()
    data = source.read()
    if isinstance(data, str):
        raise TypeError(
            "validate needs bytes, a path, or a file opened in binary mode; "
            "the file returned str"
        )
    return data


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

    def locate(self, line: Optional[int]) -> Optional[int]:
        """The segment index of pyx12's 1-based segment ``line``."""
        if line is None or not 1 <= line <= len(self.piece_starts):
            return None
        offset = self.base + self.piece_starts[line - 1]
        index = bisect_right(self.ends, offset)
        return index if index < len(self.ends) else None


class _Run:
    """What one run of pyx12 left: its verdict, the error handler it built,
    the segment numbers it finished, and the exception it raised, if any."""

    def __init__(self) -> None:
        self.ok = False
        self.handler: Any = None
        self.track: list[int] = []
        self.failure: Optional[BaseException] = None


def _run(text: str) -> _Run:
    import pyx12.params
    import pyx12.x12n_document

    run = _Run()

    def callback(_seg: Any, src: Any, _node: Any, _valid: bool) -> None:
        run.track.append(src.get_cur_line())

    with _capture.recording() as handlers:
        try:
            # the base class holds the defaults and reads no configuration file
            run.ok = pyx12.x12n_document.x12n_document(
                param=pyx12.params.ParamsBase(),
                src_file=io.StringIO(text),
                fd_997=None,
                fd_html=None,
                callback=callback,
            )
        except Exception as err:  # pyx12 is third-party: any failure is a finding
            run.failure = err
    run.handler = handlers[0] if handlers else None
    return run


def _datum(value: Any) -> bytes:
    return b"" if value is None else str(value).encode("latin-1", "replace")


def _diagnostic(found: Finding, positions: _Positions, document: Any) -> Diagnostic:
    segment = positions.locate(found.line)
    datum = _datum(found.value)
    if found.value is None and segment is not None and found.element is not None:
        separator = bytes(document.delimiters.component)
        datum = _element_bytes(document[segment], found.element, found.component, separator)
    return _external_diagnostic(
        _ORIGIN,
        found.text,
        found.level,
        code=found.code,
        segment=segment,
        element=found.element,
        component=found.component,
        datum=datum,
    )


def _element_bytes(
    segment: Any, element: int, component: Optional[int], separator: bytes
) -> bytes:
    """The bytes of a 1-based element, or of one of its components, of
    ``segment``, a composite's components joined by ``separator``; empty
    when the segment has no such position."""
    elements = segment.elements
    if not 1 <= element <= len(elements):
        return b""
    value = elements[element - 1]
    if isinstance(value, list):
        if component is None:
            return separator.join(value)
        return value[component - 1] if 1 <= component <= len(value) else b""
    return value if component in (None, 1) else b""


def _completed(track: list[int], positions: _Positions) -> Optional[int]:
    return positions.locate(track[-1]) if track else None


def _failure(reason: str, track: list[int], positions: _Positions, document: Any) -> Diagnostic:
    """One failure finding, with no code: pyx12 did not report it, it stopped.

    ``track`` holds the segment numbers pyx12 finished. When a segment
    follows the last of them, that is the one pyx12 was working on; when
    none does, pyx12 had read the whole file and the finding names the last
    segment it completed."""
    done = _completed(track, positions)
    current = positions.locate(track[-1] + 1 if track else 1)
    if current is not None:
        segment: Optional[int] = current
        where = f"it was processing segment #{current}" + (
            f"; the last it completed is #{done}" if done is not None else ""
        )
    elif done is not None:
        segment = done
        where = f"it had read every segment; the last it completed is #{done}"
    else:
        segment = None
        where = "it reached no segment"
    datum = bytes(document[segment].id) if segment is not None else b""
    return _external_diagnostic(
        _ORIGIN,
        f"could not finish validating: {reason}; {where}",
        _FAILURE_LEVEL,
        segment=segment,
        datum=datum,
    )


def _rejection(text: str, positions: _Positions, document: Any) -> Diagnostic:
    """The failure for a file pyx12 refuses to read as X12: at the segment
    holding the ISA, with its bytes as the datum. pyx12 only logs that it
    refused, so the reason is read again from its reader."""
    import pyx12.errors
    import pyx12.rawx12file

    reason = "pyx12 does not read it as an X12 file"
    try:
        pyx12.rawx12file.RawX12File(io.StringIO(text))
    except pyx12.errors.X12Error as err:
        reason = f"X12Error: {str(err).strip()}"
    segment = positions.locate(1)
    datum = b""
    if segment is not None:
        raw = bytes(document[segment].raw)
        datum = raw[max(raw.find(b"ISA"), 0):].rstrip(b"\r\n")
    where = f"it rejected segment #{segment}" if segment is not None else "it reached no segment"
    return _external_diagnostic(
        _ORIGIN,
        f"could not finish validating: {reason}; {where}",
        _FAILURE_LEVEL,
        segment=segment,
        datum=datum,
    )


def _by_segment(diagnostic: Diagnostic) -> tuple[bool, int]:
    return (diagnostic.segment is None, diagnostic.segment or 0)


def validate(source: Source) -> list[Diagnostic]:
    """Validates an 835 file with pyx12 and returns what it finds.

    ``source`` is the file's bytes, a path, or a file object open in binary
    mode. pyx12 picks its map from the file's own version declaration.
    pyx12's own defaults apply: extended character set (``charset="E"``),
    no external code lists excluded (``exclude_external_codes=None``),
    and user/system configuration files (``~/.pyx12.conf.xml``,
    ``<prefix>/etc/pyx12.conf.xml``) are not read, so findings do not
    depend on the machine.

    Every error pyx12's engine records is returned: interchange, group and
    transaction errors, segment errors and element errors, including those
    of the envelope segments. They are read from the error tree pyx12
    builds; pyx12's logging is not used and no logging configuration is
    changed, so loggers, levels, handlers and ``logging.disable`` stay as
    the caller set them.

    Each finding is an :class:`oxedi.Diagnostic`, the type ``parse``
    returns, so both lists mix, sort by ``level`` and filter by ``origin``.
    A pyx12 finding has ``kind == "External"``, ``origin == "pyx12"`` and
    ``code`` set to pyx12's own error code; ``rule`` is pyx12's message.
    ``level`` follows where pyx12 reports it: interchange, group and
    transaction findings are level 1, segment and element findings level 2.
    ``segment`` is the index of the segment at fault in the file's document
    (``None`` when pyx12 names none), and ``document[d.segment].span`` gives
    its byte range. ``datum`` is the value pyx12 reports, or else the
    element's bytes in the file. Findings come in segment order.

    A file pyx12 cannot read, or an exception inside pyx12, gives one
    level 1 finding with no ``code`` whose ``rule`` starts with ``could
    not finish validating``, instead of raising: a file pyx12 rejects
    points at the segment holding the ISA with its bytes as the datum; a
    failure while reading names the segment pyx12 was processing, and one
    after the last segment names the last segment it completed. A file
    with no ISA to read the delimiters from raises ``oxedi.ParseError``,
    as ``oxedi.parse`` does. Nothing is written to disk and no
    acknowledgement is generated.
    """
    _import_pyx12()
    data = _read(source)
    document = parse(data).document
    base = data.find(b"ISA")
    text = data[max(base, 0):].decode("latin-1")
    positions = _Positions.build(document, max(base, 0), text)
    run = _run(text)
    if run.failure is not None:
        reason = f"{type(run.failure).__name__}: {run.failure}"
        return [_failure(reason, run.track, positions, document)]
    if run.handler is None:
        return [_failure("pyx12 built no error handler", run.track, positions, document)]
    try:
        found = collect(run.handler)
    except Exception as err:  # pyx12's tree is not ours: report it, do not raise
        reason = f"its error tree could not be read ({type(err).__name__}: {err})"
        return [_failure(reason, run.track, positions, document)]
    if not found and not run.ok and not run.track:
        return [_rejection(text, positions, document)]
    out = [_diagnostic(f, positions, document) for f in found]
    return sorted(out, key=_by_segment)
