"""Cross-checks an 835 spec against pyx12's maps.

Reads pyx12's installed maps (the transaction map, ``dataele.xml`` and
``codes.xml``), located through the installed package and never copied into
the repository, and a spec composed by oxedi (``Spec.builtin`` of the
version, or ``--spec`` with its ``--spec-patch`` files; oxedi must be
installed). It compares loops (ids, parents, triggers,
maximum repeat), the segments each loop holds, the occurrences of each loop,
element usage, types, lengths and code lists, and writes:

- a Markdown report: per category, what matches, what differs and what the
  spec lacks, plus what is left out on purpose and why;
- a draft RFC 7386 merge patch that would close the differences. It is a
  patch over the spec that was compared (for 4010, the merged spec), meant to
  be reviewed by a person before any of it reaches a spec file.

Segment definitions in a spec are global: one definition per segment id,
wherever the segment appears. pyx12 describes a segment once per place it can
appear. The comparison therefore merges every place a segment is used in the
map: an element is required when every place requires it, and its code list
is the union of the lists of every place, or open (no list) when any place
leaves it open or points at an external code set. That union never rejects a
value some place accepts.

Occurrences are generated from the map, one per place a segment takes in a
loop (places marked not used are left out): its segment, position, usage
(``required`` for R, ``situational`` for S), ``max`` from ``max_use`` (none
for ``>1``), and, when the segment has several places in the loop, a
qualifier on the element pyx12 reads to tell them apart (the first element
when it is an ID with valid codes, else the first component of a composite
first element, or HL03 for HL, as pyx12 does) with that place's codes. An occurrence also carries its own
code list for each element whose list at that place is narrower than the
spec's list for the element, unless that element's codes are ignored on
purpose. Positions are the map's, 4010 ones scaled to 5010 numbering, with
the transaction's wrapper tables offset by ``table * 10000`` so the
transaction keeps one order. Names come from the map's segment names in
snake case, unique within the loop; an occurrence the spec already has keeps
the spec's name (``--fresh-names`` proposes every name from the map). The
spec's occurrences are matched to the map's by segment and identifying codes,
never by name, so a renamed occurrence still matches.

``--check`` compares only what the spec already defines and exits 1 when any
of it disagrees with the map, naming loop, segment, element and both values.
Findings listed in ``crates/oxedi_core/scripts/spec_vs_pyx12.ignore.json`` (each with its
reason) are skipped.

Usage:
    python crates/oxedi_core/scripts/spec_vs_pyx12.py [--version 5010|4010] [--report PATH] [--patch PATH]
        [--fresh-names]
    python crates/oxedi_core/scripts/spec_vs_pyx12.py --check [--version 5010|4010]

The comparison itself lives in the ``pyx12_compare`` package next to this file.
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from pyx12_compare import (  # noqa: E402,F401  (the names callers and tests read from here)
    VERSIONS,
    Source,
    load_map,
    main,
    map_position as _position,
    merge_diff,
)

if __name__ == "__main__":
    sys.exit(main())
