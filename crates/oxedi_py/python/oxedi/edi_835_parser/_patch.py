"""The compat spec: the built-in stack with the edi-835-parser patch package on top."""

from __future__ import annotations

import functools
import importlib.resources

from .. import Spec

PACKAGE: str = (
    importlib.resources.files(__package__)
    .joinpath("edi_835_parser.json")
    .read_text(encoding="utf-8")
)
"""The edi-835-parser patch package over the shipped stack."""


@functools.cache
def spec() -> Spec:
    """The shipped stack with the edi-835-parser tables in place of the native ones."""
    return Spec.from_packages(PACKAGE, Spec.builtin_packages())


TABLES = spec().tables()
