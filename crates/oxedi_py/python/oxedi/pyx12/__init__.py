"""Validation of an 835 file with pyx12, reported as oxedi835 diagnostics.

Needs the ``pyx12`` extra: ``pip install "oxedi835[pyx12]"``. ``parse`` never
calls pyx12; validating with it is an explicit call to :func:`validate`.
"""

from ._validate import validate

__all__ = ["validate"]
