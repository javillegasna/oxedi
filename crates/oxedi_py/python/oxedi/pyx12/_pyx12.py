"""The only module of the package that imports pyx12; what it hands out is
text and numbers."""

from __future__ import annotations

import io
import logging
from dataclasses import dataclass
from typing import Any, Optional


def import_pyx12() -> None:
    """Imports pyx12. Its logger gets a ``NullHandler`` when it has no handler,
    so Python's last-resort handler does not print pyx12's records."""
    try:
        import pyx12.error_handler
        import pyx12.params
        import pyx12.x12n_document
    except ImportError as err:
        raise ImportError(
            "oxedi.pyx12 needs the pyx12 package: pip install 'oxedi[pyx12]'"
        ) from err
    logger = logging.getLogger("pyx12")
    if not logger.handlers:
        logger.addHandler(logging.NullHandler())


@dataclass(frozen=True)
class Run:
    """What one run of pyx12 left."""

    report: Optional[str]
    """The JSON text pyx12 wrote, ``None`` when it wrote none."""
    track: tuple[int, ...]
    """The 1-based segment numbers pyx12 finished, in order."""
    ok: bool
    """pyx12's verdict."""
    failure: Optional[BaseException]
    """The exception pyx12 raised, if any."""


def run(text: str) -> Run:
    """Validates ``text`` with pyx12, writing its JSON error output."""
    import pyx12.params
    import pyx12.x12n_document

    track: list[int] = []
    buffer = io.StringIO()

    def callback(_seg: Any, src: Any, _node: Any, _valid: bool) -> None:
        track.append(src.get_cur_line())

    ok = False
    failure: Optional[BaseException] = None
    try:
        # the base class holds the defaults and reads no configuration file
        ok = pyx12.x12n_document.x12n_document(
            param=pyx12.params.ParamsBase(),
            src_file=io.StringIO(text),
            fd_997=None,
            fd_html=None,
            fd_json=buffer,
            callback=callback,
        )
    except Exception as err:  # pyx12 is third-party: any failure is returned
        failure = err
    return Run(buffer.getvalue() or None, tuple(track), ok, failure)


def refusal(text: str) -> str:
    """Why pyx12's reader refuses ``text``; pyx12 only logs that it did."""
    import pyx12.errors
    import pyx12.rawx12file

    try:
        pyx12.rawx12file.RawX12File(io.StringIO(text))
    except pyx12.errors.X12Error as err:
        return f"X12Error: {str(err).strip()}"
    return "pyx12 does not read it as an X12 file"
