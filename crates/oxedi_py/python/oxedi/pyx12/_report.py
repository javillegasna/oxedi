"""The JSON ``x12valid -J`` and ``x12n_document(fd_json=...)`` write, read
into frozen records that fail loudly when its shape changes."""

from __future__ import annotations

import json
from dataclasses import dataclass
from functools import partial
from typing import Callable, Optional, TypeVar, cast

T = TypeVar("T")


class PyX12ShapeError(ValueError):
    """pyx12's JSON error output does not have the expected shape."""


@dataclass(frozen=True)
class Error:
    """One error pyx12 recorded against a node."""

    code: str
    name: str
    text: str
    value: Optional[str]


@dataclass(frozen=True)
class ElementNode:
    """An element with errors, or the node that holds them."""

    position: int
    """1-based element position in its segment."""
    component: Optional[int]
    """1-based component position in a composite, or None for a simple element."""
    errors: tuple[Error, ...]


@dataclass(frozen=True)
class SegmentNode:
    """A segment with its errors and elements."""

    line: int
    """1-based line (segment count) pyx12 reports for it."""
    errors: tuple[Error, ...]
    elements: tuple[ElementNode, ...]


@dataclass(frozen=True)
class Transaction:
    """A transaction set (ST to SE) with its errors and children."""

    line: int
    """1-based line (segment count) pyx12 reports; the trailer's once it has been read."""
    errors: tuple[Error, ...]
    segments: tuple[SegmentNode, ...]


@dataclass(frozen=True)
class Group:
    """A functional group (GS to GE) with its errors and children."""

    line: int
    """1-based line (segment count) pyx12 reports; the trailer's once it has been read."""
    errors: tuple[Error, ...]
    transactions: tuple[Transaction, ...]


@dataclass(frozen=True)
class Interchange:
    """An interchange (ISA to IEA) with its errors and children."""

    line: int
    """1-based line (segment count) pyx12 reports; the trailer's once it has been read."""
    errors: tuple[Error, ...]
    groups: tuple[Group, ...]


@dataclass(frozen=True)
class Report:
    """Everything pyx12 reported, one entry per interchange."""
    interchanges: tuple[Interchange, ...]


def _node(value: object, where: str) -> dict[str, object]:
    if not isinstance(value, dict):
        raise PyX12ShapeError(f"pyx12 JSON: {where} is {type(value).__name__}, expected object")
    return value


def _field(d: dict[str, object], key: str, typ: type[T], where: str) -> T:
    if key not in d:
        raise PyX12ShapeError(f"pyx12 JSON: {key!r} is missing at {where}")
    value = d[key]
    # bool is an int in Python, but never a position or a line
    if not isinstance(value, typ) or (typ is int and isinstance(value, bool)):
        raise PyX12ShapeError(
            f"pyx12 JSON: {key!r} at {where} is {type(value).__name__}, expected {typ.__name__}"
        )
    return cast(T, value)


def _optional(d: dict[str, object], key: str, typ: type[T], where: str) -> Optional[T]:
    return None if d.get(key) is None else _field(d, key, typ, where)


def _items(
    d: dict[str, object], key: str, where: str, build: Callable[[object, str], T]
) -> tuple[T, ...]:
    base = key if where == "root" else f"{where}.{key}"
    return tuple(build(item, f"{base}[{i}]") for i, item in enumerate(_field(d, key, list, where)))


def _error(value: object, where: str) -> Error:
    d = _node(value, where)
    name = _field(d, "err_cde", str, where)
    code = _optional(d, "x12_code", str, where)
    text = _field(d, "err_str", str, where)
    return Error(name if code is None else code, name, text, _optional(d, "err_val", str, where))


def _element(value: object, where: str) -> ElementNode:
    d = _node(value, where)
    position, component = _field(d, "ele_pos", int, where), _optional(d, "subele_pos", int, where)
    return ElementNode(position, component, _items(d, "errors", where, _error))


# Each level: its record and the key of its list; the last holds elements.
_LEVELS: tuple[tuple[Callable[..., object], str], ...] = (
    (Interchange, "groups"), (Group, "transactions"),
    (Transaction, "segments"), (SegmentNode, "elements"),
)


def _level(depth: int, value: object, where: str) -> object:
    record, key = _LEVELS[depth]
    d = _node(value, where)
    last = depth + 1 == len(_LEVELS)
    child: Callable[[object, str], object] = _element if last else partial(_level, depth + 1)
    line = _field(d, "cur_line", int, where)
    return record(line, _items(d, "errors", where, _error), _items(d, key, where, child))


def parse_report(text: str) -> Report:
    """The typed JSON text pyx12 wrote; :class:`PyX12ShapeError` otherwise."""
    try:
        data = json.loads(text)
    except json.JSONDecodeError as err:
        raise PyX12ShapeError(f"pyx12 JSON: not valid JSON: {err}") from err
    items = _items(_node(data, "root"), "interchanges", "root", partial(_level, 0))
    return Report(cast(tuple[Interchange, ...], items))
