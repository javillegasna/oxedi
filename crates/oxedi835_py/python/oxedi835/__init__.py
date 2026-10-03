"""Lossless, fast, data-driven EDI 835 parser.

``parse`` reads a whole file into a document, typed tables and diagnostics.
``stream`` yields the tables in batches, one per closed loop instance.
Tables export to Arrow through the PyCapsule interface, so Polars, pyarrow
or DuckDB read them without copying.
"""

from ._core import Spec, SpecError

__all__ = ["Spec", "SpecError"]
