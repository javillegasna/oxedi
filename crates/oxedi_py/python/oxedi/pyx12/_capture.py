"""Hands ``validate`` the error tree pyx12 builds while it validates.

pyx12 creates its error handler inside ``x12n_document`` by looking up
``pyx12.error_handler.err_handler`` at call time and keeps the tree to
itself. :func:`install` replaces that name once with a subclass that
behaves as the original; a handler constructed on a thread inside
:func:`recording` also lists itself there, and notes two things the
original tree loses: where each element error of an envelope segment was
raised, and the segment errors it cannot attach to any node. Handlers
built on other threads, or outside :func:`recording`, only behave as the
original. :func:`install` also gives the ``pyx12`` logger a ``NullHandler``
when it has no handler, so its records are not printed when nothing is
configured; levels, propagation and ``logging.disable`` are not touched.
"""

from __future__ import annotations

import contextlib
import logging
import threading
from dataclasses import dataclass
from typing import Any, Iterator, Optional

_SLOT = threading.local()
_INSTALL = threading.Lock()
_MARK = "_oxedi_recorder"


@dataclass(frozen=True)
class Dropped:
    """A segment error pyx12 logs but cannot attach to its tree.

    ``line`` is the 1-based segment number pyx12 gives it, ``None`` when it
    gives none."""

    code: str
    text: str
    value: Optional[str]
    line: Optional[int]


def install(module: Any) -> None:
    """Puts the recording handler in ``module`` (``pyx12.error_handler``)
    unless it is already there; a reloaded module gets it again. The
    ``pyx12`` logger gets a ``NullHandler`` when it has no handler, so the
    records pyx12 logs reach only the handlers the caller configures and are
    not printed by Python's last-resort handler."""
    with _INSTALL:
        logger = logging.getLogger("pyx12")
        if not logger.handlers:
            logger.addHandler(logging.NullHandler())
        current = module.err_handler
        if getattr(current, _MARK, False):
            return
        module.err_handler = _recorder(current)


def _recorder(base: type) -> type:
    """A subclass of pyx12's handler class ``base`` that records for
    :func:`recording` and otherwise behaves as ``base``."""

    class Recorder(base):  # type: ignore[misc, valid-type]
        def __init__(self, *args: Any, **kwargs: Any) -> None:
            super().__init__(*args, **kwargs)
            handlers = getattr(_SLOT, "handlers", None)
            self._oxedi_recording = handlers is not None
            self.oxedi_dropped: list[Dropped] = []
            self.oxedi_element_lines: dict[int, int] = {}
            if handlers is not None:
                handlers.append(self)

        def seg_error(
            self,
            err_cde: str,
            err_str: str,
            err_value: Optional[str] = None,
            src_line: Optional[int] = None,
        ) -> None:
            node = self.cur_seg_node
            errors = getattr(node, "errors", None)
            before = len(errors) if isinstance(errors, list) else None
            super().seg_error(err_cde, err_str, err_value, src_line)
            if not self._oxedi_recording:
                return
            if before is not None and isinstance(errors, list) and len(errors) > before:
                return
            line = src_line if src_line else (node.get_cur_line() if node is not None else None)
            self.oxedi_dropped.append(Dropped(err_cde, err_str, err_value, line))

        def ele_error(
            self,
            err_cde: str,
            err_str: str,
            bad_value: Optional[str],
            refdes: Optional[str] = None,
        ) -> None:
            super().ele_error(err_cde, err_str, bad_value, refdes)
            if self._oxedi_recording and self.cur_seg_node is not None:
                self.oxedi_element_lines[id(self.cur_ele_node)] = self.cur_seg_node.get_cur_line()

    setattr(Recorder, _MARK, True)
    Recorder.__name__ = base.__name__
    Recorder.__qualname__ = base.__qualname__
    Recorder.__module__ = base.__module__
    Recorder.__doc__ = base.__doc__
    return Recorder


@contextlib.contextmanager
def recording() -> Iterator[list[Any]]:
    """Lists, in order, the handlers pyx12 creates on this thread inside
    the block."""
    handlers: list[Any] = []
    previous = getattr(_SLOT, "handlers", None)
    _SLOT.handlers = handlers
    try:
        yield handlers
    finally:
        _SLOT.handlers = previous
