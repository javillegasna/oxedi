"""Times oxedi.parse against edi_835_parser.parse on the files both can read.

Not a gate: it needs `uv pip install edi-835-parser` in the active environment and
reports and skips every file that either parser cannot read.
"""

import sys
import time
import warnings
from pathlib import Path

try:
    import edi_835_parser
    from edi_835_parser.segments import organization
    from edi_835_parser.segments.utilities import split_segment
except ImportError:
    sys.exit("edi_835_parser is not installed: uv pip install edi-835-parser")

import oxedi

warnings.simplefilter("ignore")  # the old parser warns on every unhandled segment

# edi_835_parser reads the payer id (N104) with int(), which rejects the
# alphanumeric ids of real files. This replacement of Organization.__init__ is the
# original with that one field kept as text; nothing else of the parser changes.
def _organization_init(self, segment: str):
    self.segment = segment
    fields = split_segment(segment)
    self.identifier = fields[0]
    self.type = fields[1]
    self.name = fields[2]
    self.identification_code = fields[4] if len(fields) >= 5 else None


organization.Organization.__init__ = _organization_init

tests = Path(__file__).resolve().parents[1] / "crates" / "oxedi_core" / "tests"


def files():
    for folder in ("fixtures", "samples"):
        yield from sorted(
            (p for p in (tests / folder).iterdir() if p.is_file() and p.suffix != ".md"),
            key=lambda p: p.name,
        )


def median_seconds(work, runs=5):
    times = []
    for _ in range(runs):
        start = time.perf_counter()
        work()
        times.append(time.perf_counter() - start)
    return sorted(times)[len(times) // 2]


print(f"{'file':42} {'KiB':>8} {'old ms':>10} {'oxedi ms':>12} {'speed-up':>9}")
for path in files():
    try:
        old = median_seconds(lambda: edi_835_parser.parse(str(path)).to_dataframe())
    except Exception as error:
        print(f"{path.name:42} skipped: {type(error).__name__}: {error}")
        continue
    try:
        new = median_seconds(lambda: oxedi.parse_file(path))
    except oxedi.ParseError as error:
        print(f"{path.name:42} skipped: {error}")
        continue
    kib = path.stat().st_size / 1024
    print(
        f"{path.name:42} {kib:8.1f} {old * 1000:10.1f} {new * 1000:12.1f} {old / new:8.0f}x"
    )
