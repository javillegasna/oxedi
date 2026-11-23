"""The comparison of element usage, types, lengths and code lists, and the draft definitions."""

from __future__ import annotations

from .config import LOOP_MAP
from .findings import aggregate, element_ref, snake_case

# Returned by a facet comparison to end the element's comparison.
STOP = object()


class ElementsMixin:
    """Usage, types, lengths and codes."""

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
                ref = element_ref(seg_id, position)
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
                    ref = element_ref(seg_id, position, int(comp_key))
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
        name = snake_case(agg.name)
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
        ref = element_ref(seg_id, position, component)
        loops = ",".join(agg.loops)
        path = ["segments", seg_id, "elements", str(position)]
        if component is not None:
            path += ["composite", str(component)]
        for external in agg.externals:
            self.externals.setdefault(external, set()).add(ref)
        proposal = (names, merged, position)
        for facet in (self.compare_presence, self.compare_usage, self.compare_shape,
                      self.compare_codes):
            if facet(ref, loops, path, agg, spec_def, proposal) is STOP:
                return

    def compare_presence(self, ref, loops, path, agg, spec_def, proposal):
        """Whether the spec defines the element and pyx12 uses it; STOP when
        either is missing, since nothing else is comparable then."""
        if spec_def is None:
            if agg.used:
                self.add("usage", "lacks", f"usage:{ref}", loops,
                         f"{ref} ({agg.name}) has no definition in the spec")
                self.set_patch(path, self.definition(agg, *proposal))
            return STOP
        if not agg.used:
            self.add("usage", "note", f"usage:{ref}", loops,
                     f"{ref} is defined in the spec; pyx12 marks it not used everywhere")
            return STOP

    def compare_usage(self, ref, loops, path, agg, spec_def, proposal):
        required = bool(spec_def.get("required", False))
        self.check("usage", ref, loops, path + ["required"], "required", required,
                   agg.required)

    def compare_shape(self, ref, loops, path, agg, spec_def, proposal):
        """Composite agreement, or the type and lengths of a simple element;
        STOP after a composite, which has no code list."""
        if agg.composite or spec_def.get("composite"):
            if agg.composite != bool(spec_def.get("composite")):
                self.add("types", "differs", f"types:{ref}", loops,
                         f"{ref}: composite is {bool(spec_def.get('composite'))} in the spec, "
                         f"{agg.composite} in pyx12", bool(spec_def.get("composite")),
                         agg.composite, defined=True)
            else:
                self.add("types", "match", f"types:{ref}", loops, "composite")
            return STOP
        if agg.data_ele in self.dataele:
            kind, low, high = self.dataele[agg.data_ele]
            self.check("types", ref, loops, path + ["type"], "type", spec_def.get("type"), kind)
            self.check("lengths", ref, loops, path + ["min"], "min", spec_def.get("min"), low)
            self.check("lengths", ref, loops, path + ["max"], "max", spec_def.get("max"), high)
        else:
            self.add("types", "note", f"types:{ref}", loops,
                     f"{ref}: data element {agg.data_ele!r} is not in dataele.xml; type and "
                     "lengths are not compared")

    def compare_codes(self, ref, loops, path, agg, spec_def, proposal):
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
