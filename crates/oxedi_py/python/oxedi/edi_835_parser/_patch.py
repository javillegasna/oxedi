"""The compat spec: the built-in stack with the edi-835-parser patch package on top."""

from __future__ import annotations

import functools

from .. import Spec
from .._core import EDI_835_PARSER_PACKAGE


@functools.cache
def spec() -> Spec:
    """The shipped stack with the edi-835-parser tables in place of the native ones."""
    return Spec.from_packages(EDI_835_PARSER_PACKAGE, Spec.builtin_packages())


TABLES = spec().tables()
