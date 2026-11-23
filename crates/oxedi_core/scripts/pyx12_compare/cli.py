"""The command line."""

from __future__ import annotations

import argparse
import dataclasses
import functools
import json
import sys
from pathlib import Path

from .comparison import Comparison, compare
from .config import IGNORE, OUT, VERSIONS
from .maps import pyx12_maps
from .report import failure_text, failures, report
from .spec import Source, load_ignore


DESCRIPTION = "Cross-checks an 835 spec against pyx12's maps."


@dataclasses.dataclass(frozen=True)
class Run:
    """One comparison and what its outputs name: the spec's label and the map's file name."""

    comparison: Comparison
    ignored: dict
    spec_label: str
    map_name: str


def parser():
    parser = argparse.ArgumentParser(description=DESCRIPTION)
    parser.add_argument("--version", choices=sorted(VERSIONS), default="5010")
    parser.add_argument("--spec", type=Path,
                        help="spec JSON (default: the built-in 835 of --version)")
    parser.add_argument("--spec-patch", type=Path, action="append", default=[],
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
    return parser


def compared_spec(args):
    """The built-in spec of ``--version``, or ``--spec``, with every
    ``--spec-patch`` applied over it in order."""
    base = Source.builtin(args.version) if args.spec is None else Source.file(args.spec)
    return functools.reduce(Source.patched, args.spec_patch, base)


def map_files(args):
    """The transaction map, ``dataele.xml`` and ``codes.xml``: each path given,
    else that file of pyx12's installed maps (looked up only when one is missing)."""
    installed = functools.cache(pyx12_maps)
    given = (args.map, args.dataele, args.codes)
    defaults = (VERSIONS[args.version], "dataele.xml", "codes.xml")
    return [path if path is not None else installed() / name
            for path, name in zip(given, defaults)]


def check(run):
    failed = failures(run.comparison, run.ignored)
    for finding in failed:
        print(failure_text(finding), file=sys.stderr)
    if failed:
        print(f"{len(failed)} disagreement(s) between {run.spec_label} and {run.map_name}",
              file=sys.stderr)
    return 1 if failed else 0


def write(run, args):
    report_path = args.report or OUT / f"report-{args.version}.md"
    patch_path = args.patch or OUT / f"patch-{args.version}.json"
    for path in (report_path, patch_path):
        path.parent.mkdir(parents=True, exist_ok=True)
    title = f"{run.spec_label} vs pyx12 {run.map_name}"
    report_path.write_text(report(run.comparison, title, run.ignored))
    patch_path.write_text(json.dumps(run.comparison.patch, indent=2) + "\n")
    print(f"report: {report_path}\npatch: {patch_path}")
    return 0


def main(argv=None):
    args = parser().parse_args(argv)
    source = compared_spec(args)
    map_path, dataele, codes = map_files(args)
    ignored = load_ignore(args.ignore, map_path.name)
    comparison = compare(source.model(), map_path, dataele, codes, ignored, args.fresh_names)
    run = Run(comparison, ignored, source.label, map_path.name)
    return check(run) if args.check else write(run, args)
