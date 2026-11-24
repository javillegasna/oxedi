"""The JSON types a binding argument takes."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import TypeAlias, Union

JsonValue: TypeAlias = Union[
    None,
    bool,
    int,
    float,
    str,
    Sequence["JsonValue"],
    Mapping[str, "JsonValue"],
]
"""A value JSON can hold; an integer must fit in 64 bits and a float be finite.

The sequence and the mapping are read-only types, so a ``list[str]`` or a
``dict[str, list[str]]`` is a value; at run time a list or a tuple and any
registered ``Mapping`` are accepted, and ``bytes`` are refused.
"""

JsonObject: TypeAlias = Mapping[str, JsonValue]
"""A JSON object: a mapping whose keys are ``str`` and whose values are JSON values."""
