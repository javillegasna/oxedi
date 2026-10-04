"""Compares oxedi835.edi_835_parser with edi-835-parser on every file of a directory.

Not a gate. Meant for files that must not enter the repository: the report
names files by their position in the sorted listing and prints only shapes,
column names, counts and verdicts, never a value read from a file. The N104
shim of the test suite is applied so the library reads alphanumeric payer
ids. Usage: python scripts/compat_oracle.py DIR [--out REPORT]
"""

from __future__ import annotations

import argparse
import sys
import warnings
from pathlib import Path

TESTS = Path(__file__).resolve().parents[1] / "crates" / "oxedi835_py" / "tests"


def compare(path, old, new):
    """One report block for one file: a list of lines."""
    if path.read_bytes()[:3] != b"ISA":
        return ["skipped: does not start with ISA"]
    try:
        expected_sets = old.parse(str(path))
        expected = expected_sets.to_dataframe()
    except Exception as error:
        return [f"edi-835-parser failed: {type(error).__name__}"]
    try:
        actual_sets = new.parse(path)
        actual = actual_sets.to_dataframe()
    except Exception as error:
        return [f"oxedi835 failed: {type(error).__name__}"]
    lines = [f"shape edi-835-parser {expected.shape} oxedi835 {actual.shape}"]
    only_old = [c for c in expected.columns if c not in actual.columns]
    only_new = [c for c in actual.columns if c not in expected.columns]
    if only_old or only_new:
        lines.append(f"columns only in edi-835-parser: {only_old}; only in oxedi835: {only_new}")
    if list(expected.columns) != list(actual.columns) and not (only_old or only_new):
        lines.append("same columns in a different order")
    if expected.shape[0] == actual.shape[0]:
        for column in (c for c in expected.columns if c in actual.columns):
            left, right = expected[column].reset_index(drop=True), actual[column].reset_index(drop=True)
            if str(left.dtype) != str(right.dtype):
                lines.append(f"{column}: dtype {left.dtype} vs {right.dtype}")
            differs = ~((left == right) | (left.isna() & right.isna()))
            if int(differs.sum()):
                lines.append(f"{column}: {int(differs.sum())} cells differ")
    for name in ("count_claims", "count_patients", "sum_payments"):
        a, b = getattr(expected_sets, name)(), getattr(actual_sets, name)()
        if abs(a - b) > 0.005:
            lines.append(f"{name} differs")
    if len(lines) == 1:
        lines.append("equal")
    return lines


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("directory", type=Path)
    parser.add_argument("--out", type=Path, help="write the report here instead of stdout")
    args = parser.parse_args(argv)
    try:
        import edi_835_parser as old
    except ImportError:
        sys.exit('edi-835-parser is not installed: uv pip install "edi-835-parser==1.8.0"')
    sys.path.insert(0, str(TESTS))
    import n104_shim

    from oxedi835 import edi_835_parser as new

    n104_shim.apply()
    warnings.simplefilter("ignore")
    files = sorted(p for p in args.directory.iterdir() if p.is_file())
    report = []
    for number, path in enumerate(files, 1):
        report.append(f"file {number}/{len(files)}")
        report.extend(f"  {line}" for line in compare(path, old, new))
    text = "\n".join(report) + "\n"
    if args.out:
        args.out.write_text(text)
    else:
        sys.stdout.write(text)


if __name__ == "__main__":
    main()
