"""Validation of an 835 file with pyx12, reported as oxedi diagnostics.

Needs the ``pyx12`` extra: ``pip install "oxedi[pyx12]"``. ``parse`` never
calls pyx12; validating with it is an explicit call to :func:`validate`.
"""

from ._validate import validate

__all__ = ["validate"]
