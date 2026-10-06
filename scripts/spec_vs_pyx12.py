"""Cross-checks an 835 spec against pyx12's maps.

Reads pyx12's installed maps (the transaction map, ``dataele.xml`` and
``codes.xml``), located through the installed package and never copied into
the repository, and a spec (``specs/835.json``, with ``specs/835.4010.json``
merged over it for the 4010 map). It compares loops (ids, parents, triggers),
the segments each loop holds, element usage, types, lengths and code lists,
and writes:

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

``--check`` compares only what the spec already defines and exits 1 when any
of it disagrees with the map, naming loop, segment, element and both values.
Findings listed in ``scripts/spec_vs_pyx12.ignore.json`` (each with its
reason) are skipped.

Usage:
    python scripts/spec_vs_pyx12.py [--version 5010|4010] [--report PATH] [--patch PATH]
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

CATEGORIES = ("loops", "segments", "usage", "types", "lengths", "codes")


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


@dataclass
class MapLoop:
    xid: str
    kind: str | None
    parent: str | None
    segments: list[MapSegment]


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

    def walk(node, parent):
        for child in node:
            if child.tag != "loop":
                continue
            xid = child.get("xid")
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
                    )
                )
            loops.append(MapLoop(xid, child.get("type"), parent, segments))
            walk(child, xid)

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
    def __init__(self, spec, loops, dataele, codesets):
        self.spec = spec
        self.loops = loops
        self.dataele = dataele
        self.codesets = codesets
        self.findings: list[Finding] = []
        self.patch: dict = {}
        self.unmapped: list[str] = []
        self.externals: dict[str, set[str]] = {}

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
            missing = []
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
                    missing.append(seg_id)
                    self.add("segments", "lacks", f"segments:{name}:{seg_id}", name,
                             f"loop {name} does not hold {seg_id} (pyx12 usage {usage})",
                             None, usage)
            for seg_id in listed:
                if seg_id and seg_id not in segments:
                    self.add("segments", "differs", f"segments:{name}:{seg_id}", name,
                             f"loop {name} holds {seg_id} in the spec; pyx12 does not "
                             "place it there", seg_id, None, defined=True)
            for seg_id in missing:
                self.set_patch(["loops", name, "occurrences", seg_id.lower()],
                               {"segment": seg_id, "pos": 0})

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
    return f"{finding.key}: loop {finding.loops}: {finding.message}"


def compare(spec, map_path, dataele_path, codes_path):
    return Comparison(
        spec, load_map(map_path), load_dataele(dataele_path), load_codesets(codes_path)
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
    comparison = compare(
        load_spec(spec_path, patches),
        map_path,
        args.dataele or maps / "dataele.xml",
        args.codes or maps / "codes.xml",
    )
    ignored = load_ignore(args.ignore, Path(map_path).name)

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
