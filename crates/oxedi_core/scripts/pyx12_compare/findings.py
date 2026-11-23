"""The comparison's records and the helpers that normalise what is compared."""

from __future__ import annotations

import re
from dataclasses import dataclass

from .config import LOOP_MAP


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


def snake_case(name):
    return re.sub(r"[^a-z0-9]+", "_", name.lower()).strip("_") or "element"


def identity_codes(occurrence):
    """The codes that identify a spec occurrence: its qualifier's, else its
    own codes for the first element."""
    qualifier = occurrence.get("qualifier") or {}
    first = occurrence.get("elements", {}).get("1") or {}
    return set(qualifier.get("codes") or first.get("codes") or [])


def normalized(occurrence):
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
        "elements": {key: {"codes": sorted(entry["codes"])}
                     for key, entry in occurrence.get("elements", {}).items()
                     if entry.get("codes")},
    }


def element_ref(segment, position, component=None):
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
