"""Reading the compared spec through oxedi, the RFC 7386 diff the draft patch is, and the ignore list."""

from __future__ import annotations

import dataclasses
import json
import sys
from pathlib import Path

from oxedi import Spec, SpecError


@dataclasses.dataclass(frozen=True)
class Source:
    """The spec a run compares, read and patched by oxedi, and its name in the
    report title and the ``--check`` summary."""

    spec: Spec
    label: str

    @classmethod
    def builtin(cls, version):
        """The built-in spec of ``version`` (``5010`` or ``4010``), as oxedi composes it."""
        return cls(Spec.builtin(version), f"built-in {version}")

    @classmethod
    def file(cls, path):
        """The spec in the JSON file ``path``."""
        return cls(_read(path, Spec.from_json), path.name)

    def patched(self, path):
        """This spec with the merge patch in the JSON file ``path`` applied by oxedi."""
        return Source(_read(path, self.spec.patch), f"{self.label} + {path.name}")

    def model(self):
        """The spec's model as the comparison reads it: oxedi's canonical JSON,
        with each occurrence's ``usage`` written out. The canonical JSON leaves
        out the default ``situational``; the occurrences generated from the
        map write it, and the draft patch is their difference, so the spec's
        occurrences carry it too."""
        model = json.loads(self.spec.to_json())
        for loop in model.get("loops", {}).values():
            for occurrence in loop.get("occurrences", {}).values():
                occurrence.setdefault("usage", "situational")
        return model


def _read(path, load):
    """``load`` applied to the text of ``path``; a spec or patch oxedi refuses
    stops the run with the path and oxedi's reason."""
    try:
        return load(path.read_text())
    except SpecError as error:
        sys.exit(f"{path}: {error}")


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
