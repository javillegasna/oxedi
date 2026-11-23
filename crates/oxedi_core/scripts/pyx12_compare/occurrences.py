"""The occurrences a map generates for each loop, matched with the spec's and compared."""

from __future__ import annotations

import re
from collections import Counter

from .config import LOOP_MAP
from .findings import element_ref, identity_codes, normalized, snake_case
from .spec import merge_diff


def occurrence_of(seg, qualifier, codes):
    """The occurrence a map segment generates; the qualifier and the codes
    are left out when empty."""
    return {"segment": seg.id, "pos": seg.pos,
            "usage": "required" if seg.usage == "R" else "situational",
            **({} if seg.max_use is None else {"max": seg.max_use}),
            **({} if qualifier is None else {"qualifier": qualifier}),
            **({"codes": codes} if codes else {})}


class OccurrencesMixin:
    """Occurrences generated from the map, matched with the spec's, compared."""

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
                ours_normal, theirs = normalized(spec_occurrences[spec_name]), normalized(occurrence)
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
        shared = {sid for sid, n in Counter(seg.id for seg in segs).items() if n > 1}
        out = []
        for seg in segs:
            identity = self.identity(seg)
            qualifier, place = self.qualify(loop_name, seg, identity, shared)
            occurrence = occurrence_of(seg, qualifier, self.own_codes(seg, place))
            out.append((seg, occurrence, set(identity[2]) if identity else set()))
        return out

    def qualify(self, loop_name, seg, identity, shared):
        """``(qualifier, place)`` of a segment with several places in the loop:
        both None when it has one place or no identifying element (a note then)."""
        if seg.id not in shared:
            return None, None
        if identity is None:
            self.add("occurrences", "note", f"occurrences:{loop_name}:{seg.id}",
                     loop_name, f"loop {loop_name}: {seg.id} {seg.name!r} has no "
                     "identifying element to tell its places apart")
            return None, None
        element, component, codes = identity
        qualifier = {"element": element,
                     **({} if component is None else {"component": component}),
                     "codes": sorted(set(codes))}
        return qualifier, (element, component)

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
                if f"codes:{element_ref(seg.id, pos, comp)}" in self.ignored:
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
            meets = {i: [name for name, occurrence in ours if identity_codes(occurrence) & generated[i][2]]
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
            base = snake_case(seg.name) if seg.name else seg.id.lower()
            proposed, n = base, 2
            while proposed in taken:
                proposed, n = f"{base}_{n}", n + 1
            taken.add(proposed)
            names.append(proposed)
        return {name: occurrence for name, (_, occurrence, _) in zip(names, generated)}
