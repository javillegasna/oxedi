#!/usr/bin/env python3
"""Anonymize X12 835 files so they can live in a public repository.

Replaces every direct identifier with a deterministic fake value while keeping
the file structurally identical: same segments, same element counts, same
delimiters, same trivia (newlines) between segments, same ISA width. Amounts,
codes, dates and counts are untouched so the files stay useful as parser fixtures.

The same original value always maps to the same fake value (within one run and
across files), so cross-references such as a payee NPI repeated in TS3 survive.

What is replaced (element positions are X12 positions, XX01 = first element):

  ISA06, ISA08, GS02, GS03        sender/receiver ids        same-length alnum
  BPR07/09/10/11/13/15            bank routing/accounts      same-length alnum
  TRN02, TRN03, TRN04             trace number, payer ids    same-length alnum
  N102                            payer/payee name           fake organisation
  N104                            id: XX -> NPI, FI -> 9 digits, other -> alnum (XV kept)
  N3*                             street lines               fake street / PO box
  N401, N403                      city, zip                  fake city, same-length digits
  NM103..NM105                    last/org, first, middle    fake person or organisation
  NM109                           id: XX -> NPI, else same-length alnum (MI, HN, 34, FI, SY...)
  PER02, PER04/06/08              contact name, phone/email/url
  REF02                           every qualifier except F2 (version)   same-length alnum
  CLP01, CLP07                    patient account, payer claim number   same-length alnum
  PLB01, PLB03-2 (and 05-2, ...)  provider id -> NPI, reference ids -> alnum
  TS301                           provider id -> NPI

Fake NPIs are Luhn-valid with the 80840 prefix, so NPI validators still pass.

Usage:
  anonymize_835.py --in-dir /path/outside/repo/originals --out-dir crates/edi835_core/tests/samples \
                   [--mapping /path/outside/repo/mapping.json] [--seed x]

The mapping file is a re-identification key: keep it with the originals, never
in the repository.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import random
import sys
from pathlib import Path

# --------------------------------------------------------------------------- data

FIRST_NAMES = [
    "ALICE", "BRUNO", "CARLA", "DIEGO", "ELENA", "FELIX", "GRETA", "HUGO", "IRENE", "JONAS",
    "KARLA", "LEON", "MARTA", "NOAH", "OLGA", "PABLO", "QUINN", "ROSA", "SIMON", "TANIA",
    "ULISES", "VERA", "WALTER", "XENIA", "YAGO", "ZOE", "AMIR", "BEATRIZ", "CESAR", "DUNIA",
    "EMILIO", "FLORA", "GABRIEL", "HELGA", "IVAN", "JULIA", "KEVIN", "LAURA", "MATEO", "NORA",
]
LAST_NAMES = [
    "ANDRADE", "BELTRAN", "CASTRO", "DUARTE", "ESCOBAR", "FUENTES", "GALVEZ", "HERRERA", "IBARRA",
    "JIMENEZ", "KOVACS", "LOZANO", "MENDEZ", "NAVARRO", "OCHOA", "PAREDES", "QUINTERO", "RIVAS",
    "SALAZAR", "TORRES", "URIBE", "VARGAS", "WEBER", "XIMENEZ", "YANEZ", "ZAMORA", "ACOSTA",
    "BRAVO", "CORDERO", "DELGADO", "ESPINOZA", "FARIAS", "GUERRA", "HOLGUIN", "IGLESIAS",
    "JARAMILLO", "LARA", "MOLINA", "NUNEZ", "OSORIO", "PINEDA", "ROJAS", "SOTO", "TAPIA",
]
ORG_ADJ = ["NORTHERN", "COASTAL", "SUMMIT", "RIVERSIDE", "PRAIRIE", "HARBOR", "VALLEY", "PINE",
           "CEDAR", "GRANITE", "BLUE RIDGE", "LAKESIDE", "MERIDIAN", "WESTGATE", "SILVER OAK"]
ORG_NOUN = ["VISION CENTER", "EYE CARE", "MEDICAL GROUP", "HEALTH PARTNERS", "FAMILY CLINIC",
            "OPTICAL", "HEALTH PLAN", "BENEFITS CO", "CARE NETWORK", "WELLNESS ASSOCIATES"]
CITIES = ["SPRINGFIELD", "RIVERTON", "FAIRVIEW", "MADISON", "GREENVILLE", "CLINTON", "ASHLAND",
          "BRISTOL", "OAKDALE", "KINGSTON", "MILFORD", "NEWPORT", "SALEM", "WINCHESTER", "DAYTON"]
STREET_NAMES = ["MAIN", "OAK", "MAPLE", "CEDAR", "PINE", "ELM", "WASHINGTON", "LAKE", "HILL",
                "PARK", "RIVER", "SUNSET", "HIGHLAND", "MEADOW", "WILLOW"]
STREET_TYPES = ["ST", "AVE", "RD", "BLVD", "DR", "LN", "WAY"]


# ------------------------------------------------------------------ generators

class Anonymizer:
    def __init__(self, seed: str) -> None:
        self.seed = seed
        self.mapping: dict[tuple[str, bytes], bytes] = {}

    def _rng(self, category: str, value: bytes) -> random.Random:
        digest = hashlib.sha256(f"{self.seed}|{category}|".encode() + value).digest()
        return random.Random(digest)

    def replace(self, category: str, value: bytes) -> bytes:
        if not value.strip():
            return value
        key = (category, value)
        if key not in self.mapping:
            self.mapping[key] = self._generate(category, value)
        return self.mapping[key]

    def _generate(self, category: str, value: bytes) -> bytes:
        r = self._rng(category, value)
        text = value.decode("latin-1")
        if category == "first":
            return match_case(r.choice(FIRST_NAMES), text).encode("latin-1")
        if category == "last":
            return match_case(r.choice(LAST_NAMES), text).encode("latin-1")
        if category == "middle":
            if len(text.strip()) == 1:
                return r.choice("ABCDEFGHJKLMNPRSTVW").encode()
            return match_case(r.choice(FIRST_NAMES), text).encode("latin-1")
        if category == "org":
            org = f"{r.choice(ORG_ADJ)} {r.choice(ORG_NOUN)}"
            if r.random() < 0.4:
                org += " " + r.choice(["INC", "LLC", "PC", "LLP"])
            return match_case(org, text).encode("latin-1")
        if category == "city":
            return match_case(r.choice(CITIES), text).encode("latin-1")
        if category == "street":
            upper = text.upper().replace(".", "").replace(" ", "")
            if upper.startswith("POBOX") or upper.startswith("PBOX"):
                fake = f"PO BOX {r.randint(100, 99999)}"
            else:
                fake = f"{r.randint(1, 9999)} {r.choice(STREET_NAMES)} {r.choice(STREET_TYPES)}"
            return match_case(fake, text).encode("latin-1")
        if category == "npi":
            return fake_npi(r).encode()
        if category == "digits":
            return "".join(r.choice("0123456789") for _ in text).encode()
        if category == "alnum":
            return same_shape(text, r).encode("latin-1")
        if category == "email":
            return f"contact{r.randint(100, 999)}@example.com".encode()
        if category == "url":
            return f"WWW.EXAMPLE-{r.randint(100, 999)}.COM".encode()
        raise ValueError(f"unknown category {category}")


def match_case(fake: str, original: str) -> str:
    if original.isupper() or not any(c.isalpha() for c in original):
        return fake.upper()
    if original.istitle():
        return fake.title()
    if original.islower():
        return fake.lower()
    return fake.upper()


def same_shape(text: str, r: random.Random) -> str:
    """Random string with the same length and per-position character class."""
    out = []
    for ch in text:
        if ch.isdigit():
            out.append(r.choice("0123456789"))
        elif ch.isupper():
            out.append(r.choice("ABCDEFGHJKLMNPQRSTUVWXYZ"))
        elif ch.islower():
            out.append(r.choice("abcdefghjkmnpqrstuvwxyz"))
        else:
            out.append(ch)
    return "".join(out)


def fake_npi(r: random.Random) -> str:
    """Ten-digit NPI with a valid Luhn check digit over the 80840 prefix."""
    body = r.choice("12") + "".join(r.choice("0123456789") for _ in range(8))
    digits = [int(d) for d in "80840" + body]
    total = 0
    for i, d in enumerate(reversed(digits)):
        if i % 2 == 0:
            d *= 2
            if d > 9:
                d -= 9
        total += d
    check = (10 - total % 10) % 10
    return body + str(check)


# ------------------------------------------------------------------ segment rules

def anonymize_elements(e: list[bytes], comp: bytes, a: Anonymizer) -> list[bytes]:
    """`e[0]` is the segment id, `e[n]` is element n (X12 numbering)."""
    sid = e[0]

    def has(n: int) -> bool:
        return len(e) > n and e[n] != b""

    def set_(n: int, category: str) -> None:
        if has(n):
            e[n] = a.replace(category, e[n])

    def set_id(n_qual: int, n_id: int) -> None:
        if not has(n_id):
            return
        qual = e[n_qual] if len(e) > n_qual else b""
        if qual == b"XX":
            e[n_id] = a.replace("npi", e[n_id])
        elif qual == b"FI":
            e[n_id] = a.replace("digits", e[n_id])
        elif qual == b"XV":
            return  # payer id, not personal
        else:
            e[n_id] = a.replace("alnum", e[n_id])

    if sid == b"ISA":
        set_(6, "alnum")
        set_(8, "alnum")
    elif sid == b"GS":
        set_(2, "alnum")
        set_(3, "alnum")
    elif sid == b"BPR":
        for n in (7, 9, 10, 11, 13, 15):
            set_(n, "alnum")
    elif sid == b"TRN":
        set_(2, "alnum")
        set_(3, "alnum")
        set_(4, "alnum")
    elif sid == b"N1":
        set_(2, "org")
        set_id(3, 4)
    elif sid == b"N3":
        for n in range(1, len(e)):
            set_(n, "street")
    elif sid == b"N4":
        set_(1, "city")
        set_(3, "digits")
    elif sid == b"NM1":
        person = len(e) > 2 and e[2] == b"1"
        set_(3, "last" if person else "org")
        set_(4, "first")
        set_(5, "middle")
        set_id(8, 9)
    elif sid == b"PER":
        set_(2, "org")
        for n_qual, n_val in ((3, 4), (5, 6), (7, 8)):
            if has(n_val):
                qual = e[n_qual]
                if qual == b"EM":
                    set_(n_val, "email")
                elif qual == b"UR":
                    set_(n_val, "url")
                else:
                    set_(n_val, "digits")
    elif sid == b"REF":
        if len(e) > 1 and e[1] != b"F2":
            set_(2, "alnum")
    elif sid == b"CLP":
        set_(1, "alnum")
        set_(7, "alnum")
    elif sid == b"PLB":
        if has(1):
            e[1] = a.replace("npi" if is_npi_like(e[1]) else "alnum", e[1])
        for n in range(3, len(e), 2):  # PLB03, PLB05, ... are composites "reason:reference"
            if has(n):
                parts = e[n].split(comp)
                if len(parts) > 1 and parts[1]:
                    parts[1] = a.replace("alnum", parts[1])
                e[n] = comp.join(parts)
    elif sid == b"TS3":
        if has(1):
            e[1] = a.replace("npi" if is_npi_like(e[1]) else "alnum", e[1])
    return e


def is_npi_like(value: bytes) -> bool:
    return len(value) == 10 and value.isdigit()


# ------------------------------------------------------------------ framing

TRIVIA = b" \t\r\n"


def read_delimiters(data: bytes) -> tuple[bytes, bytes, bytes]:
    if not data.startswith(b"ISA"):
        raise SystemExit("input does not start with ISA; cannot read delimiters")
    sep = data[3:4]
    pos, n = 3, 1
    while n < 16:
        pos = data.index(sep, pos + 1)
        n += 1
    return sep, data[pos + 1 : pos + 2], data[pos + 2 : pos + 3]


def anonymize_file(data: bytes, a: Anonymizer) -> bytes:
    sep, comp, term = read_delimiters(data)
    out = bytearray()
    rest = data
    while rest:
        body_start = 0
        while body_start < len(rest) and rest[body_start] in TRIVIA:
            body_start += 1
        term_at = rest.find(term, body_start)
        if term_at < 0:
            body, tail, rest_next = rest[body_start:], b"", b""
        else:
            body, tail, rest_next = rest[body_start:term_at], term, rest[term_at + 1 :]
        out += rest[:body_start]
        if body:
            elements = anonymize_elements(body.split(sep), comp, a)
            out += sep.join(elements)
        out += tail
        rest = rest_next
    return bytes(out)


# ------------------------------------------------------------------ verification

def structure(data: bytes) -> list[tuple[bytes, int]]:
    sep, _, term = read_delimiters(data)
    shape = []
    for frame in data.split(term):
        body = frame.lstrip(TRIVIA)
        if body:
            e = body.split(sep)
            shape.append((e[0], len(e)))
    return shape


NAME_CATEGORIES = {"first", "last", "middle", "org", "city", "street"}


def all_values(data: bytes) -> set[bytes]:
    """Every element and composite component in the file, as whole values."""
    sep, comp, term = read_delimiters(data)
    values: set[bytes] = set()
    for frame in data.split(term):
        for element in frame.lstrip(TRIVIA).split(sep):
            values.add(element)
            values.update(element.split(comp))
    return values


def verify(original: bytes, anonymized: bytes, a: Anonymizer, name: str) -> tuple[list[str], list[str]]:
    """Returns (problems, warnings).

    A problem is an identifier (NPI, id, account...) that survives as a whole
    element. A warning is a dictionary collision: a fake name that happens to
    equal some real name in the input, which is not re-identifying because it is
    attached to different ids, claims and companions.
    """
    problems, warnings = [], []
    if structure(original) != structure(anonymized):
        problems.append(f"{name}: segment/element structure changed")
    if original.index(b"~") != anonymized.index(b"~"):
        problems.append(f"{name}: ISA width changed")
    present = all_values(anonymized)
    # Values shorter than 4 bytes are codes (facility type, LX counter...) that
    # collide with short ids by chance; they carry no identity.
    leaked_ids = sorted(
        orig for (cat, orig), fake in a.mapping.items()
        if cat not in NAME_CATEGORIES and fake != orig and len(orig) >= 4 and orig in present
    )
    if leaked_ids:
        problems.append(f"{name}: {len(leaked_ids)} identifiers still present, e.g. {leaked_ids[0]!r}")
    collisions = sum(
        1 for (cat, orig), fake in a.mapping.items()
        if cat in NAME_CATEGORIES and fake != orig and orig in present
    )
    if collisions:
        warnings.append(f"{name}: {collisions} fake names coincide with a real name in the input (dictionary collision, not linkable)")
    return problems, warnings


# ------------------------------------------------------------------ main

def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--in-dir", required=True, type=Path)
    p.add_argument("--out-dir", required=True, type=Path)
    p.add_argument("--mapping", type=Path, help="write the re-identification key here (keep it private)")
    p.add_argument("--seed", default="oxedi835")
    args = p.parse_args()

    a = Anonymizer(args.seed)
    args.out_dir.mkdir(parents=True, exist_ok=True)
    files = sorted(f for f in args.in_dir.iterdir() if f.is_file())
    if not files:
        print(f"no files in {args.in_dir}", file=sys.stderr)
        return 1

    problems: list[str] = []
    warnings: list[str] = []
    for f in files:
        original = f.read_bytes()
        anonymized = anonymize_file(original, a)
        (args.out_dir / f.name).write_bytes(anonymized)
        file_problems, file_warnings = verify(original, anonymized, a, f.name)
        problems += file_problems
        warnings += file_warnings
        print(f"{f.name}: {len(original)} -> {len(anonymized)} bytes, {len(structure(original))} segments")

    if args.mapping:
        key = {f"{cat}|{orig.decode('latin-1')}": fake.decode("latin-1") for (cat, orig), fake in a.mapping.items()}
        args.mapping.write_text(json.dumps(key, indent=1, sort_keys=True))
        print(f"mapping: {len(key)} entries -> {args.mapping}")

    for warning in warnings:
        print("WARNING:", warning, file=sys.stderr)
    for problem in problems:
        print("PROBLEM:", problem, file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
