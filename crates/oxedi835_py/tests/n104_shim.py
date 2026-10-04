"""Lets edi-835-parser read payer ids that are not numeric.

The library converts ``N104`` with ``int()``, which fails on the alphanumeric
ids that X12 allows (qualifier ``XV``). ``apply`` replaces the segment
constructor with one that keeps ``N104`` as text; nothing else changes.
"""

from edi_835_parser.segments import organization
from edi_835_parser.segments.utilities import split_segment


def _init(self, segment):
    self.segment = segment
    elements = split_segment(segment)
    self.identifier = elements[0]
    self.type = elements[1]
    self.name = elements[2]
    self.identification_code = elements[4] if len(elements) >= 5 else None


def apply():
    organization.Organization.__init__ = _init
