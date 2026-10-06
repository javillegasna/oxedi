"""Cross-checks an 835 spec against pyx12's maps.

Reads pyx12's installed maps (the transaction map, ``dataele.xml`` and
``codes.xml``), located through the installed package and never copied into
the repository, and a spec (``specs/835.json``, with ``specs/835.4010.json``
merged over it for the 4010 map). It compares loops (ids, parents, triggers,
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
Findings listed in ``scripts/spec_vs_pyx12.ignore.json`` (each with its
reason) are skipped.

Usage:
    python scripts/spec_vs_pyx12.py [--version 5010|4010] [--report PATH] [--patch PATH]
        [--fresh-names]
    python scripts/spec_vs_pyx12.py --check [--version 5010|4010]
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import xml.etree.ElementTree as ET
from dataclasses import dataclass, field
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
SPECS = REPO / "crates" / "oxedi_core" / "specs"
IGNORE = Path(__file__).resolve().with_name("spec_vs_pyx12.ignore.json")
OUT = REPO / ".superpowers" / "spec_vs_pyx12"

VERSIONS = {
    "5010": {"map": "835.5010.X221.A1.xml", "patches": []},
    "4010": {"map": "835.4010.X091.A1.xml", "patches": ["835.4010.json"]},
}

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


# ---- pyx12 maps ----------------------------------------------------------


@dataclass
class MapElement:
    """One element (or component) of one segment at one place in the map."""

    seq: int
    data_ele: str
    name: str
    usage: str
    # None: any value (no list, an empty list, or an external code set).
    codes: tuple[str, ...] | None
    external: str | None
    components: dict[int, MapElement] = field(default_factory=dict)

    @property
    def composite(self):
        return bool(self.components)


@dataclass
class MapSegment:
    """One place a segment appears in the map."""

    id: str
    name: str
    usage: str
    loop: str
    elements: dict[int, MapElement]
    # Position in its spec loop (see ``_position``); max_use, None for ">1".
    pos: int = 0
    max_use: int | None = None


@dataclass
class MapLoop:
    xid: str
    kind: str | None
    parent: str | None
    segments: list[MapSegment]
    # The loop's repeat; None for ">1" or when the map gives none.
    repeat: int | None = None
    usage: str = ""


def _count(text):
    """A map count: ``>1`` (or nothing) is no limit, otherwise the number."""
    text = (text or "").strip()
    return int(text) if text.isdigit() else None


def _position(text, table=0):
    """A map position as the spec writes it. 4010 maps write positions with
    three digits and 5010 maps with four (``030`` is ``0300``), so a 4010
    position is scaled by ten and both versions number alike. The wrapper
    tables of a transaction (header, detail, footer) restart their
    positions, so a segment inside the n-th table, or inside a loop nested in
    it, gets ``n * 10000`` added: the transaction and every loop below it
    share one position space."""
    text = (text or "").strip()
    pos = int(text) if text.isdigit() else 0
    if len(text) == 3:
        pos *= 10
    return table * 10000 + pos


def _element(node):
    codes_node = node.find("valid_codes")
    external = None
    codes = None
    if codes_node is not None:
        external = codes_node.get("external")
        # An empty <code/> names no value; a list of only empty codes is open.
        listed = tuple(
            text for text in ((code.text or "").strip() for code in codes_node.findall("code"))
            if text
        )
        if external is None and listed:
            codes = listed
    element = MapElement(
        seq=int(node.findtext("seq")),
        data_ele=node.findtext("data_ele") or "",
        name=node.findtext("name") or "",
        usage=node.findtext("usage") or "",
        codes=codes,
        external=external,
    )
    if node.tag == "composite":
        for child in node.findall("element"):
            component = _element(child)
            element.components[component.seq] = component
    return element


def load_map(path):
    """Every loop of a pyx12 transaction map, in document order."""
    root = ET.parse(path).getroot()
    loops = []

    def walk(node, parent, inherited=0):
        tables = 0
        for child in node:
            if child.tag != "loop":
                continue
            xid = child.get("xid")
            table = inherited
            if child.get("type") == "wrapper":
                tables += 1
                table = tables
            segments = []
            for seg in child.findall("segment"):
                elements = {}
                for el in seg:
                    if el.tag in ("element", "composite"):
                        parsed = _element(el)
                        elements[parsed.seq] = parsed
                segments.append(
                    MapSegment(
                        id=seg.get("xid"),
                        name=seg.findtext("name") or "",
                        usage=seg.findtext("usage") or "",
                        loop=xid,
                        elements=elements,
                        pos=_position(seg.findtext("pos"), table),
                        max_use=_count(seg.findtext("max_use")),
                    )
                )
            loops.append(MapLoop(xid, child.get("type"), parent, segments,
                                 _count(child.findtext("repeat")),
                                 (child.findtext("usage") or "").strip()))
            walk(child, xid, table)

    walk(root, None)
    return loops


def load_dataele(path):
    """Data element number -> (type, min, max)."""
    root = ET.parse(path).getroot()
    return {
        node.get("ele_num"): (
            node.get("data_type"),
            int(node.get("min_len")),
            int(node.get("max_len")),
        )
        for node in root.iter("data_ele")
    }


def load_codesets(path):
    """External code set id -> number of codes ``codes.xml`` lists for it."""
    root = ET.parse(path).getroot()
    return {
        node.findtext("id"): len(node.findall("./version/code"))
        for node in root.iter("codeset")
    }


def pyx12_maps():
    """The folder of the installed pyx12 maps; exits with a message when
    pyx12 is not installed."""
    try:
        from importlib.resources import files

        return Path(str(files("pyx12") / "map"))
    except ModuleNotFoundError:
        sys.exit("pyx12 is not installed: pip install 'pyx12>=4.0,<5'")


# ---- spec ----------------------------------------------------------------


def merge_patch(target, patch):
    """RFC 7386: objects merge key by key, null deletes, anything else replaces."""
    if not isinstance(patch, dict):
        return patch
    result = dict(target) if isinstance(target, dict) else {}
    for key, value in patch.items():
        if value is None:
            result.pop(key, None)
        else:
            result[key] = merge_patch(result.get(key), value)
    return result


def load_spec(path, patches=()):
    spec = json.loads(Path(path).read_text())
    for patch in patches:
        spec = merge_patch(spec, json.loads(Path(patch).read_text()))
    return spec


# ---- comparison ----------------------------------------------------------


@dataclass
class Finding:
    category: str
    # "match", "differs" (the spec says something else), "lacks" (the spec
    # says nothing) or "note" (worth knowing, nothing to change).
    status: str
    key: str
    loops: str
    message: str
    spec_value: object = None
    map_value: object = None
    # True when the spec defines what disagrees: --check fails on it.
    defined: bool = False


@dataclass
class Aggregate:
    """An element merged over every place its segment is used in the map."""

    name: str
    data_ele: str
    used: bool
    required: bool
    codes: tuple[str, ...] | None
    externals: set[str]
    composite: bool
    loops: list[str]


def _snake(name):
    return re.sub(r"[^a-z0-9]+", "_", name.lower()).strip("_") or "element"


def merge_diff(base, target):
    """The RFC 7386 merge patch that turns ``base`` into ``target``."""
    if not isinstance(base, dict) or not isinstance(target, dict):
        return target
    out = {key: None for key in base if key not in target}
    for key, value in target.items():
        if key not in base:
            out[key] = value
        elif base[key] != value:
            both = isinstance(base[key], dict) and isinstance(value, dict)
            out[key] = merge_diff(base[key], value) if both else value
    return out


def _ident(occurrence):
    """The codes that identify a spec occurrence: its qualifier's, else its
    own codes for the first element."""
    qualifier = occurrence.get("qualifier") or {}
    return set(qualifier.get("codes") or occurrence.get("codes", {}).get("1") or [])


def _normal(occurrence):
    """An occurrence with its defaults written out, for comparison."""
    qualifier = occurrence.get("qualifier")
    if qualifier is not None:
        qualifier = {"element": qualifier.get("element"),
                     "component": qualifier.get("component"),
                     "codes": sorted(qualifier.get("codes", []))}
    return {
        "segment": occurrence.get("segment"),
        "pos": occurrence.get("pos"),
        "usage": occurrence.get("usage", "situational"),
        "max": occurrence.get("max"),
        "qualifier": qualifier,
        "codes": {key: sorted(codes) for key, codes in occurrence.get("codes", {}).items()},
    }


def _ref(segment, position, component=None):
    text = f"{segment}{position:02d}"
    return text if component is None else f"{text}-{component}"


def aggregate(uses):
    """Merges the elements of every used place of one segment id:
    {(position, component or None): Aggregate}."""
    places = {}
    for use in uses:
        if use.usage == "N":
            continue
        for seq, element in use.elements.items():
            places.setdefault((seq, None), []).append((use, element, None))
            for cseq, component in element.components.items():
                places.setdefault((seq, cseq), []).append((use, component, element))
    merged = {}
    for key, entries in sorted(places.items(), key=lambda kv: (kv[0][0], kv[0][1] or 0)):
        live = [
            (use, el)
            for use, el, parent in entries
            if el.usage != "N" and (parent is None or parent.usage != "N")
        ]
        first = entries[0][1]
        open_list = any(el.codes is None for _, el in live)
        codes = None
        if live and not open_list:
            codes = tuple(sorted({code for _, el in live for code in el.codes}))
        merged[key] = Aggregate(
            name=first.name,
            data_ele=first.data_ele,
            used=bool(live),
            required=bool(live) and all(el.usage == "R" for _, el in live),
            codes=codes,
            externals={el.external for _, el in live if el.external},
            composite=first.composite,
            loops=sorted({LOOP_MAP.get(use.loop, use.loop)
                          for use, *_ in (live or entries)}),
        )
    return merged


class Comparison:
    def __init__(self, spec, loops, dataele, codesets, ignored=None, fresh_names=False):
        self.spec = spec
        self.loops = loops
        self.dataele = dataele
        self.codesets = codesets
        # Ignored finding keys: a code list kept apart from the map on purpose
        # is not narrowed per occurrence either.
        self.ignored = ignored or {}
        # Propose every occurrence name from the map instead of keeping the
        # names the spec already gives its occurrences.
        self.fresh_names = fresh_names
        self.findings: list[Finding] = []
        self.patch: dict = {}
        self.unmapped: list[str] = []
        self.externals: dict[str, set[str]] = {}
        # Spec loop name -> {occurrence name: occurrence} generated from the map.
        self.generated: dict[str, dict] = {}

    def add(self, category, status, key, loops, message, spec_value=None, map_value=None,
            defined=False):
        self.findings.append(
            Finding(category, status, key, loops, message, spec_value, map_value, defined)
        )

    def set_patch(self, path, value):
        node = self.patch
        for part in path[:-1]:
            node = node.setdefault(part, {})
        node[path[-1]] = value

    def run(self):
        self.compare_loops()
        self.compare_segments()
        self.compare_occurrences()
        self.compare_elements()
        return self

    # loops: ids, parents, triggers
    def compare_loops(self):
        spec_loops = self.spec.get("loops", {})
        for loop in self.loops:
            if loop.xid not in LOOP_MAP:
                self.unmapped.append(loop.xid)
        for name in sorted(set(spec_loops) - set(LOOP_MAP.values())):
            self.add("loops", "differs", f"loops:{name}", name,
                     f"spec loop {name!r} has no pyx12 loop in the mapping table",
                     defined=True)
        for loop in self.loops:
            name = LOOP_MAP.get(loop.xid)
            if name is None or loop.kind == "wrapper":
                continue
            ours = spec_loops.get(name)
            if ours is None:
                self.add("loops", "lacks", f"loops:{name}", name,
                         f"pyx12 loop {loop.xid} ({name}) is not in the spec")
                continue
            parent = LOOP_MAP.get(loop.parent) if loop.parent else None
            if ours.get("parent") == parent:
                self.add("loops", "match", f"loops:{name}:parent", name, "parent")
            else:
                self.add("loops", "differs", f"loops:{name}:parent", name,
                         f"loop {name}: parent is {ours.get('parent')!r} in the spec, "
                         f"{parent!r} in pyx12", ours.get("parent"), parent, defined=True)
                self.set_patch(["loops", name, "parent"], parent)
            first = loop.segments[0] if loop.segments else None
            trigger = ours.get("trigger", {})
            if first is None or trigger.get("segment") != first.id:
                self.add("loops", "differs", f"loops:{name}:trigger", name,
                         f"loop {name}: trigger segment is {trigger.get('segment')!r} in "
                         f"the spec, {first.id if first else None!r} in pyx12",
                         trigger.get("segment"), first.id if first else None, defined=True)
                continue
            ok = True
            for position, value in sorted(trigger.get("where", {}).items()):
                element = first.elements.get(int(position))
                allowed = element.codes if element else None
                if allowed is not None and value not in allowed:
                    ok = False
                    self.add("loops", "differs", f"loops:{name}:trigger:{position}", name,
                             f"loop {name}: trigger {_ref(first.id, int(position))} = "
                             f"{value!r} in the spec; pyx12 allows {list(allowed)}",
                             value, list(allowed), defined=True)
            if ok:
                self.add("loops", "match", f"loops:{name}:trigger", name, "trigger")

    # segments per loop
    def compare_segments(self):
        spec_loops = self.spec.get("loops", {})
        held = {}
        for loop in self.loops:
            name = LOOP_MAP.get(loop.xid)
            if name is None:
                continue
            for seg in loop.segments:
                held.setdefault(name, {}).setdefault(seg.id, []).append(seg.usage)
        for name, segments in held.items():
            ours = spec_loops.get(name)
            if ours is None:
                continue
            listed = [ours.get("trigger", {}).get("segment")]
            listed += [o.get("segment") for o in ours.get("occurrences", {}).values()]
            listed += [ours["end"]] if ours.get("end") else []
            for seg_id, usages in segments.items():
                usage = "R" if "R" in usages else ("S" if "S" in usages else "N")
                if seg_id in listed:
                    status = "match" if usage != "N" else "differs"
                    self.add("segments", status, f"segments:{name}:{seg_id}", name,
                             f"loop {name} holds {seg_id} (pyx12 usage {usage})"
                             if status == "match" else
                             f"loop {name} holds {seg_id}; pyx12 marks it not used",
                             seg_id, usage, defined=status != "match")
                elif usage != "N":
                    self.add("segments", "lacks", f"segments:{name}:{seg_id}", name,
                             f"loop {name} does not hold {seg_id} (pyx12 usage {usage})",
                             None, usage)
            for seg_id in listed:
                if seg_id and seg_id not in segments:
                    self.add("segments", "differs", f"segments:{name}:{seg_id}", name,
                             f"loop {name} holds {seg_id} in the spec; pyx12 does not "
                             "place it there", seg_id, None, defined=True)

    # occurrences: generated from the map, matched with the spec's, compared
    def compare_occurrences(self):
        spec_loops = self.spec.get("loops", {})
        held, repeat, usage = {}, {}, {}
        for loop in self.loops:
            name = LOOP_MAP.get(loop.xid)
            if name is None or name not in spec_loops:
                continue
            if loop.kind != "wrapper":
                repeat[name] = loop.repeat
                usage[name] = "required" if loop.usage == "R" else "situational"
            end = spec_loops[name].get("end")
            for seg in loop.segments:
                if seg.id != end and seg.usage != "N":
                    held.setdefault(name, []).append(seg)
        for name, segs in held.items():
            ours = spec_loops[name]
            spec_occurrences = ours.get("occurrences", {})
            declared = bool(spec_occurrences)
            status = "differs" if declared else "lacks"
            generated = self.generate(name, sorted(segs, key=lambda seg: seg.pos))
            pairs = {} if self.fresh_names else self.match(spec_occurrences, generated)
            named = self.name(generated, pairs)
            self.generated[name] = named
            if not declared:
                self.add("occurrences", "differs", f"occurrences:{name}", name,
                         f"loop {name} declares no occurrences; pyx12 places {len(named)} "
                         "in it", None, list(named), defined=True)
            if ours.get("usage", "situational") == usage.get(name):
                self.add("occurrences", "match", f"occurrences:{name}:usage", name, "loop usage")
            else:
                self.add("occurrences", status, f"occurrences:{name}:usage", name,
                         f"loop {name}: usage is {ours.get('usage', 'situational')!r} in the "
                         f"spec, {usage.get(name)!r} in pyx12", ours.get("usage"),
                         usage.get(name), defined=declared)
                self.set_patch(["loops", name, "usage"], usage.get(name))
            if ours.get("max") == repeat.get(name):
                self.add("occurrences", "match", f"occurrences:{name}:max", name, "loop max")
            else:
                self.add("occurrences", status, f"occurrences:{name}:max", name,
                         f"loop {name}: max is {ours.get('max')!r} in the spec, "
                         f"{repeat.get(name)!r} in pyx12", ours.get("max"), repeat.get(name),
                         defined=declared)
                self.set_patch(["loops", name, "max"], repeat.get(name))
            proposed = list(named)
            for index, (seg, occurrence, _) in enumerate(generated):
                spec_name = pairs.get(index)
                if spec_name is None:
                    self.add("occurrences", status, f"occurrences:{name}:{proposed[index]}", name,
                             f"loop {name}: pyx12 occurrence {proposed[index]} ({seg.id} "
                             f"{seg.name!r}) has no counterpart in the spec",
                             None, occurrence, defined=declared)
                    continue
                ours_normal, theirs = _normal(spec_occurrences[spec_name]), _normal(occurrence)
                for field_name, value in ours_normal.items():
                    key = f"occurrences:{name}:{spec_name}:{field_name}"
                    if value == theirs[field_name]:
                        self.add("occurrences", "match", key, name, field_name)
                    else:
                        self.add("occurrences", "differs", key, name,
                                 f"loop {name} occurrence {spec_name}: {field_name} is {value!r} "
                                 f"in the spec, {theirs[field_name]!r} in pyx12",
                                 value, theirs[field_name], defined=True)
            for spec_name in spec_occurrences:
                if spec_name not in pairs.values():
                    self.add("occurrences", "differs", f"occurrences:{name}:{spec_name}", name,
                             f"loop {name}: spec occurrence {spec_name} has no counterpart in "
                             "pyx12", spec_name, None, defined=True)
            diff = merge_diff(spec_occurrences, named)
            if diff:
                self.set_patch(["loops", name, "occurrences"], diff)

    def identity(self, seg):
        """Where pyx12 reads the code that tells a segment's places apart, as
        ``(element, component, codes)``; None when it has no such place."""
        first = seg.elements.get(1)

        def kind(element):
            return self.dataele.get(element.data_ele, (None,))[0]

        if first is not None and first.composite:
            component = first.components.get(1)
            if component is not None and component.codes and kind(component) == "ID":
                return 1, 1, component.codes
        elif first is not None and first.codes and kind(first) == "ID":
            return 1, None, first.codes
        third = seg.elements.get(3)
        if seg.id == "HL" and third is not None and third.codes:
            return 3, None, third.codes
        return None

    def generate(self, loop_name, segs):
        """[(map segment, occurrence, identifying codes)] in position order. A
        segment with several places in the loop gets a qualifier."""
        counts = {}
        for seg in segs:
            counts[seg.id] = counts.get(seg.id, 0) + 1
        out = []
        for seg in segs:
            occurrence = {"segment": seg.id, "pos": seg.pos,
                          "usage": "required" if seg.usage == "R" else "situational"}
            if seg.max_use is not None:
                occurrence["max"] = seg.max_use
            identity = self.identity(seg)
            place = None
            if counts[seg.id] > 1:
                if identity is None:
                    self.add("occurrences", "note", f"occurrences:{loop_name}:{seg.id}",
                             loop_name, f"loop {loop_name}: {seg.id} {seg.name!r} has no "
                             "identifying element to tell its places apart")
                else:
                    element, component, codes = identity
                    place = (element, component)
                    qualifier = {"element": element}
                    if component is not None:
                        qualifier["component"] = component
                    qualifier["codes"] = sorted(set(codes))
                    occurrence["qualifier"] = qualifier
            codes = self.own_codes(seg, place)
            if codes:
                occurrence["codes"] = codes
            out.append((seg, occurrence, set(identity[2]) if identity else set()))
        return out

    def own_codes(self, seg, place):
        """The closed code lists of this place that are narrower than the
        spec's list for the element, by codes key; the qualifier's place and
        elements the spec does not define, or keeps apart on purpose, are left out."""
        definitions = self.spec.get("segments", {}).get(seg.id, {}).get("elements", {})
        out = {}
        for position, element in sorted(seg.elements.items()):
            if element.usage == "N":
                continue
            places = [(position, None, element, definitions.get(str(position)))]
            for cseq, component in sorted(element.components.items()):
                parent = definitions.get(str(position)) or {}
                places.append((position, cseq, component,
                               (parent.get("composite") or {}).get(str(cseq))))
            for pos, comp, node, spec_def in places:
                if node.usage == "N" or node.codes is None or spec_def is None:
                    continue
                if (pos, comp) == place or spec_def.get("composite"):
                    continue
                kind = spec_def.get("type", "")
                if kind == "R" or re.fullmatch(r"N\d", kind):
                    continue
                if f"codes:{_ref(seg.id, pos, comp)}" in self.ignored:
                    continue
                mine = sorted(set(node.codes))
                if spec_def.get("codes") is not None and sorted(spec_def["codes"]) == mine:
                    continue
                out[str(pos) if comp is None else f"{pos}-{comp}"] = mine
        return out

    @staticmethod
    def match(spec_occurrences, generated):
        """{generated index: spec occurrence name}: the only place of a segment
        on both sides, or the one place whose identifying codes meet exactly
        one occurrence's, and that occurrence's only those."""
        pairs = {}
        for segment in {occurrence["segment"] for _, occurrence, _ in generated}:
            ours = [(name, occurrence) for name, occurrence in spec_occurrences.items()
                    if occurrence.get("segment") == segment]
            theirs = [i for i, (_, occurrence, _) in enumerate(generated)
                      if occurrence["segment"] == segment]
            if len(ours) == 1 and len(theirs) == 1:
                pairs[theirs[0]] = ours[0][0]
                continue
            meets = {i: [name for name, occurrence in ours if _ident(occurrence) & generated[i][2]]
                     for i in theirs}
            for i, names in meets.items():
                if len(names) == 1 and [j for j in theirs if names[0] in meets[j]] == [i]:
                    pairs[i] = names[0]
        return pairs

    @staticmethod
    def name(generated, pairs):
        """{name: occurrence} in position order: the spec's name for a matched
        place, otherwise the map's segment name in snake case, unique in the loop."""
        taken = set(pairs.values())
        names = []
        for index, (seg, _, _) in enumerate(generated):
            if index in pairs:
                names.append(pairs[index])
                continue
            base = _snake(seg.name) if seg.name else seg.id.lower()
            proposed, n = base, 2
            while proposed in taken:
                proposed, n = f"{base}_{n}", n + 1
            taken.add(proposed)
            names.append(proposed)
        return {name: occurrence for name, (_, occurrence, _) in zip(names, generated)}

    # usage, types, lengths, codes
    def compare_elements(self):
        spec_segments = self.spec.get("segments", {})
        uses = {}
        for loop in self.loops:
            if loop.xid in LOOP_MAP:
                for seg in loop.segments:
                    uses.setdefault(seg.id, []).append(seg)
        for seg_id in sorted(set(spec_segments) - set(uses)):
            self.add("segments", "differs", f"segments:{seg_id}", "-",
                     f"segment {seg_id} is defined in the spec; no mapped pyx12 loop holds it",
                     seg_id, None, defined=True)
        for seg_id, places in uses.items():
            merged = aggregate(places)
            ours = spec_segments.get(seg_id)
            if ours is None:
                if any(use.usage != "N" for use in places):
                    loops = ",".join(sorted({LOOP_MAP[use.loop] for use in places}))
                    self.add("segments", "lacks", f"segments:{seg_id}", loops,
                             f"segment {seg_id} has no element definitions in the spec")
                    self.set_patch(["segments", seg_id], {"elements": self.draft(merged)})
                continue
            elements = ours.get("elements", {})
            names = {d.get("name") for d in elements.values()}
            self.spec_only(seg_id, elements, merged, places)
            for (position, component), agg in merged.items():
                if component is not None:
                    continue
                spec_def = elements.get(str(position))
                self.compare_element(seg_id, position, None, agg, spec_def, names, merged)
                if agg.composite and spec_def is not None and spec_def.get("composite"):
                    comps = spec_def["composite"]
                    comp_names = {d.get("name") for d in comps.values()}
                    for (pos, comp), cagg in merged.items():
                        if pos == position and comp is not None:
                            self.compare_element(seg_id, position, comp, cagg,
                                                 comps.get(str(comp)), comp_names, merged)

    def spec_only(self, seg_id, elements, merged, places):
        """Elements and components the spec defines at positions the map
        never lists for the segment."""
        loops = ",".join(sorted({LOOP_MAP[use.loop] for use in places}))
        for key, spec_def in elements.items():
            position = int(key)
            if (position, None) not in merged:
                ref = _ref(seg_id, position)
                self.add("usage", "differs", f"usage:{ref}", loops,
                         f"{ref} is defined in the spec; pyx12 lists no such position",
                         spec_def.get("name"), None, defined=True)
                continue
            if not merged[(position, None)].used:
                # pyx12 does not detail the components of an unused composite;
                # the element itself is reported as a note.
                continue
            for comp_key, comp_def in (spec_def.get("composite") or {}).items():
                if (position, int(comp_key)) not in merged:
                    ref = _ref(seg_id, position, int(comp_key))
                    self.add("usage", "differs", f"usage:{ref}", loops,
                             f"{ref} is defined in the spec; pyx12 lists no such position",
                             comp_def.get("name"), None, defined=True)

    def draft(self, merged, position=None):
        """Element definitions for every used element (or, with ``position``,
        every used component of that composite)."""
        out = {}
        names = set()
        for (pos, comp), agg in merged.items():
            if not agg.used:
                continue
            if position is None and comp is not None:
                continue
            if position is not None and (pos != position or comp is None):
                continue
            key = str(pos if position is None else comp)
            out[key] = self.definition(agg, names, merged, pos)
        return out

    def definition(self, agg, names, merged, position):
        name = _snake(agg.name)
        base, n = name, 2
        while name in names:
            name, n = f"{base}_{n}", n + 1
        names.add(name)
        if agg.composite:
            return {"name": name, "type": "AN", **({"required": True} if agg.required else {}),
                    "composite": self.draft(merged, position)}
        # Without a dataele entry the type is a guess and no lengths are set.
        kind, low, high = self.dataele.get(agg.data_ele, ("AN", None, None))
        out = {"name": name, "type": kind}
        if agg.required:
            out["required"] = True
        if low is not None:
            out["min"], out["max"] = low, high
        if agg.codes:
            out["codes"] = list(agg.codes)
        return out

    def compare_element(self, seg_id, position, component, agg, spec_def, names, merged):
        ref = _ref(seg_id, position, component)
        loops = ",".join(agg.loops)
        path = ["segments", seg_id, "elements", str(position)]
        if component is not None:
            path += ["composite", str(component)]
        for external in agg.externals:
            self.externals.setdefault(external, set()).add(ref)
        if spec_def is None:
            if agg.used:
                self.add("usage", "lacks", f"usage:{ref}", loops,
                         f"{ref} ({agg.name}) has no definition in the spec")
                self.set_patch(path, self.definition(agg, names, merged, position))
            return
        if not agg.used:
            self.add("usage", "note", f"usage:{ref}", loops,
                     f"{ref} is defined in the spec; pyx12 marks it not used everywhere")
            return
        required = bool(spec_def.get("required", False))
        self.check("usage", ref, loops, path + ["required"], "required", required,
                   agg.required)
        if agg.composite or spec_def.get("composite"):
            if agg.composite != bool(spec_def.get("composite")):
                self.add("types", "differs", f"types:{ref}", loops,
                         f"{ref}: composite is {bool(spec_def.get('composite'))} in the spec, "
                         f"{agg.composite} in pyx12", bool(spec_def.get("composite")),
                         agg.composite, defined=True)
            else:
                self.add("types", "match", f"types:{ref}", loops, "composite")
            return
        if agg.data_ele in self.dataele:
            kind, low, high = self.dataele[agg.data_ele]
            self.check("types", ref, loops, path + ["type"], "type", spec_def.get("type"), kind)
            self.check("lengths", ref, loops, path + ["min"], "min", spec_def.get("min"), low)
            self.check("lengths", ref, loops, path + ["max"], "max", spec_def.get("max"), high)
        else:
            self.add("types", "note", f"types:{ref}", loops,
                     f"{ref}: data element {agg.data_ele!r} is not in dataele.xml; type and "
                     "lengths are not compared")
        spec_codes = spec_def.get("codes")
        map_codes = list(agg.codes) if agg.codes is not None else None
        if spec_codes is None and map_codes is None:
            self.add("codes", "match", f"codes:{ref}", loops, "open")
        elif spec_codes is None:
            self.add("codes", "lacks", f"codes:{ref}", loops,
                     f"{ref} ({agg.name}) has no code list in the spec; pyx12 lists "
                     f"{len(map_codes)}", None, map_codes)
            self.set_patch(path + ["codes"], map_codes)
        elif sorted(spec_codes) == map_codes:
            self.add("codes", "match", f"codes:{ref}", loops, "codes")
        else:
            self.add("codes", "differs", f"codes:{ref}", loops,
                     f"{ref}: codes are {sorted(spec_codes)} in the spec, "
                     f"{map_codes if map_codes is not None else 'open'} in pyx12",
                     sorted(spec_codes), map_codes, defined=True)
            self.set_patch(path + ["codes"], map_codes)

    def check(self, category, ref, loops, path, what, spec_value, map_value):
        if spec_value == map_value:
            self.add(category, "match", f"{category}:{ref}:{what}", loops, what)
            return
        self.add(category, "differs", f"{category}:{ref}:{what}", loops,
                 f"{ref}: {what} is {spec_value!r} in the spec, {map_value!r} in pyx12",
                 spec_value, map_value, defined=True)
        self.set_patch(path, map_value)


# ---- ignore list, report, check -----------------------------------------


def load_ignore(path, map_name):
    """{key: reason} for the entries that apply to ``map_name``."""
    if not Path(path).exists():
        return {}
    entries = json.loads(Path(path).read_text()).get("ignore", [])
    out = {}
    for i, entry in enumerate(entries):
        reason = (entry.get("reason") or "").strip()
        if not entry.get("key") or not reason:
            sys.exit(f"{path}: ignore[{i}] needs a non-empty \"key\" and \"reason\"")
        if entry.get("map") in (None, map_name):
            out[entry["key"]] = reason
    return out


def report(comparison, title, ignored):
    lines = [f"# {title}", ""]
    lines += [
        "Draft patch: a merge patch over the compared spec; review every entry before",
        "applying it. Code lists merge every place a segment is used (see the script).",
        "",
        "| category | match | differs | lacks | note | ignored |",
        "|---|---|---|---|---|---|",
    ]
    for category in CATEGORIES:
        found = [f for f in comparison.findings if f.category == category]
        count = {s: sum(1 for f in found if f.status == s and f.key not in ignored)
                 for s in ("match", "differs", "lacks", "note")}
        skipped = sum(1 for f in found if f.key in ignored)
        lines.append(f"| {category} | {count['match']} | {count['differs']} | "
                     f"{count['lacks']} | {count['note']} | {skipped} |")
    for category in CATEGORIES:
        for status, heading in (("differs", "differs"), ("lacks", "the spec lacks"),
                                ("note", "notes")):
            found = [f for f in comparison.findings
                     if f.category == category and f.status == status and f.key not in ignored]
            if not found:
                continue
            lines += ["", f"## {category}: {heading}", ""]
            lines += [f"- `{f.key}` [{f.loops}] {f.message}" for f in found]
    lines += ["", "## Occurrences generated from the map", ""]
    for name, occurrences in comparison.generated.items():
        lines.append(f"- {name}: " + "; ".join(
            f"`{occ_name}` {occ['segment']}@{occ['pos']}"
            + (f" {occ['qualifier']['codes']}" if occ.get("qualifier") else "")
            + f" {occ['usage'][0].upper()}" + (f" max {occ['max']}" if "max" in occ else "")
            for occ_name, occ in occurrences.items()))
    lines += ["", "## Left out on purpose", ""]
    for name, refs in sorted(comparison.externals.items()):
        size = comparison.codesets.get(name)
        listed = f"codes.xml lists {size}" if size is not None else "not in codes.xml"
        lines.append(f"- external code set `{name}` ({listed}) on {', '.join(sorted(refs))}: "
                     "external code sets are not copied into the spec")
    for key, reason in sorted(ignored.items()):
        lines.append(f"- `{key}`: {reason}")
    if comparison.unmapped:
        lines += ["", "## Unmapped pyx12 loops", ""]
        lines += [f"- {xid}" for xid in comparison.unmapped]
    return "\n".join(lines) + "\n"


def failures(comparison, ignored):
    return [f for f in comparison.findings
            if f.status == "differs" and f.defined and f.key not in ignored]


def failure_text(finding):
    # A message that already names its loop is not prefixed with it again.
    if finding.message.startswith(f"loop {finding.loops}"):
        return f"{finding.key}: {finding.message}"
    return f"{finding.key}: loop {finding.loops}: {finding.message}"


def compare(spec, map_path, dataele_path, codes_path, ignored=None, fresh_names=False):
    return Comparison(
        spec, load_map(map_path), load_dataele(dataele_path), load_codesets(codes_path),
        ignored, fresh_names,
    ).run()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--version", choices=sorted(VERSIONS), default="5010")
    parser.add_argument("--spec", type=Path, help="spec JSON (default: the built-in 835)")
    parser.add_argument("--spec-patch", type=Path, action="append",
                        help="merge patch applied over --spec (repeatable)")
    parser.add_argument("--map", type=Path, help="pyx12 transaction map")
    parser.add_argument("--dataele", type=Path, help="pyx12 dataele.xml")
    parser.add_argument("--codes", type=Path, help="pyx12 codes.xml")
    parser.add_argument("--ignore", type=Path, default=IGNORE)
    parser.add_argument("--report", type=Path)
    parser.add_argument("--patch", type=Path)
    parser.add_argument("--check", action="store_true",
                        help="exit 1 when what the spec defines disagrees with the map")
    parser.add_argument("--fresh-names", action="store_true",
                        help="propose every occurrence name from the map instead of keeping "
                             "the spec's names")
    args = parser.parse_args(argv)

    version = VERSIONS[args.version]
    if args.spec is None:
        spec_path = SPECS / "835.json"
        patches = [SPECS / name for name in version["patches"]]
    else:
        spec_path, patches = args.spec, []
    patches += args.spec_patch or []
    needs_maps = args.map is None or args.dataele is None or args.codes is None
    maps = pyx12_maps() if needs_maps else None
    map_path = args.map or maps / version["map"]
    ignored = load_ignore(args.ignore, Path(map_path).name)
    comparison = compare(
        load_spec(spec_path, patches),
        map_path,
        args.dataele or maps / "dataele.xml",
        args.codes or maps / "codes.xml",
        ignored,
        args.fresh_names,
    )

    if args.check:
        failed = failures(comparison, ignored)
        for finding in failed:
            print(failure_text(finding), file=sys.stderr)
        if failed:
            print(f"{len(failed)} disagreement(s) between {spec_path.name} and "
                  f"{Path(map_path).name}", file=sys.stderr)
            return 1
        return 0

    report_path = args.report or OUT / f"report-{args.version}.md"
    patch_path = args.patch or OUT / f"patch-{args.version}.json"
    report_path.parent.mkdir(parents=True, exist_ok=True)
    patch_path.parent.mkdir(parents=True, exist_ok=True)
    title = f"{spec_path.name}{''.join(' + ' + Path(p).name for p in patches)} vs pyx12 " \
            f"{Path(map_path).name}"
    report_path.write_text(report(comparison, title, ignored))
    patch_path.write_text(json.dumps(comparison.patch, indent=2) + "\n")
    print(f"report: {report_path}\npatch: {patch_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
