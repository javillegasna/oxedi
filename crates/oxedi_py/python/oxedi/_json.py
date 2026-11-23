"""The JSON types a binding argument takes."""

from __future__ import annotations

from collections.abc import Mapping
from typing import TypeAlias, Union

JsonValue: TypeAlias = Union[
    None,
    bool,
    int,
    float,
    str,
    "list[JsonValue]",
    "tuple[JsonValue, ...]",
    "Mapping[str, JsonValue]",
]
"""A value JSON can hold; an integer must fit in 64 bits and a float be finite."""

JsonObject: TypeAlias = Mapping[str, JsonValue]
"""A JSON object: a mapping whose keys are ``str`` and whose values are JSON values."""
