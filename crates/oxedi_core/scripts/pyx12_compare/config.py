"""Paths, the versions the script compares (version -> map file) and the loop mapping table."""

from __future__ import annotations

from pathlib import Path


REPO = Path(__file__).resolve().parents[4]
IGNORE = Path(__file__).resolve().parents[1] / "spec_vs_pyx12.ignore.json"
OUT = REPO / "target" / "spec_vs_pyx12"

VERSIONS = {"5010": "835.5010.X221.A1.xml", "4010": "835.4010.X091.A1.xml"}

# pyx12 loop id -> spec loop name. The wrapper loops of the transaction
# (HEADER, DETAIL, FOOTER) hold segments and loops the spec places directly
# in the transaction.
LOOP_MAP = {
    "ISA_LOOP": "interchange",
    "GS_LOOP": "group",
    "ST_LOOP": "transaction",
    "HEADER": "transaction",
    "DETAIL": "transaction",
    "FOOTER": "transaction",
    "1000A": "1000A",
    "1000B": "1000B",
    "2000": "2000",
    "2100": "2100",
    "2110": "2110",
}

CATEGORIES = ("loops", "segments", "occurrences", "usage", "types", "lengths", "codes")
