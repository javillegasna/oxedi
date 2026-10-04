"""Checks a built wheel's packaging metadata against the repository root.

usage: check_wheel.py WHEEL ROOT

Fails unless the wheel's dist-info/licenses/ holds LICENSE and THIRD_PARTY_NOTICES
byte-identical to the files in ROOT, and every Requires-Dist is tied to an extra
(the base install has no Python dependencies).
"""

import sys
import zipfile
from email.parser import BytesParser
from pathlib import Path


def main(wheel: str, root: str) -> int:
    problems = []
    with zipfile.ZipFile(wheel) as zf:
        names = zf.namelist()
        for name in ("LICENSE", "THIRD_PARTY_NOTICES"):
            members = [n for n in names if n.endswith(f".dist-info/licenses/{name}")]
            if len(members) != 1:
                problems.append(f"{wheel}: expected one dist-info/licenses/{name}, found {members}")
            elif zf.read(members[0]) != (Path(root) / name).read_bytes():
                problems.append(f"{wheel}: {members[0]} differs from {root}/{name}")
        metadata = [n for n in names if n.endswith(".dist-info/METADATA")]
        if len(metadata) != 1:
            problems.append(f"{wheel}: expected one METADATA, found {metadata}")
        else:
            headers = BytesParser().parsebytes(zf.read(metadata[0]), headersonly=True)
            for requirement in headers.get_all("Requires-Dist") or []:
                if "extra ==" not in requirement:
                    problems.append(f"{wheel}: base install requires {requirement!r}")
    for problem in problems:
        print(f"check_wheel: {problem}", file=sys.stderr)
    if not problems:
        print(f"check_wheel: ok ({Path(wheel).name})")
    return 1 if problems else 0


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    sys.exit(main(sys.argv[1], sys.argv[2]))
