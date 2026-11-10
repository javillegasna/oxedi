"""What ``oxedi.pyx12.validate`` finds in each file, pinned as JSON goldens.

A golden holds one row per finding, in segment order: level, X12 code,
segment, element, component, datum and message. A change in what pyx12 or
the binding reports shows as a diff of the golden; read it, then commit it.
Regenerate with ``make sandbox CMD="UPDATE_GOLDEN=1 make py-test"``.
"""

import json
import os
import re

import pytest

pytest.importorskip("pyx12")

from oxedi.pyx12 import validate

from conftest import VALIDATED, pyx12_golden, read

KEYS = ("level", "code", "segment", "element", "component", "datum", "rule")
REGENERATE = 'make sandbox CMD="UPDATE_GOLDEN=1 make py-test"'


def rows(findings):
    return [
        {
            "level": d.level,
            "code": d.code,
            "segment": d.segment,
            "element": d.element,
            "component": d.component,
            "datum": d.datum.decode("utf-8"),
            "rule": d.rule,
        }
        for d in findings
    ]


def render(findings):
    return json.dumps(rows(findings), indent=1) + "\n"


def expected(golden):
    try:
        return golden.read_text(encoding="utf-8")
    except FileNotFoundError:
        raise AssertionError(
            f"{golden}: no golden; run {REGENERATE} to create it, then read it and commit it"
        ) from None


def check(golden, actual):
    if os.environ.get("UPDATE_GOLDEN") == "1":
        golden.parent.mkdir(parents=True, exist_ok=True)
        golden.write_text(actual, encoding="utf-8")
    assert actual == expected(golden)


@pytest.mark.parametrize("name", VALIDATED)
def test_validate_finds_what_the_golden_pins(name):
    check(pyx12_golden(name), render(validate(read(name))))


def test_a_missing_golden_names_the_command_that_creates_it(tmp_path, monkeypatch):
    monkeypatch.delenv("UPDATE_GOLDEN", raising=False)
    golden = tmp_path / "x.json"
    with pytest.raises(AssertionError, match=re.escape(REGENERATE)) as info:
        check(golden, "[]\n")
    assert str(golden) in str(info.value)


def test_update_golden_writes_what_the_comparison_reads(tmp_path, monkeypatch):
    golden = tmp_path / "new" / "x.json"
    text = render(validate(read("united_healthcare_legacy_sample.txt")))
    monkeypatch.setenv("UPDATE_GOLDEN", "1")
    check(golden, text)
    monkeypatch.delenv("UPDATE_GOLDEN")
    check(golden, text)
    written = json.loads(golden.read_text())
    assert written[0]["datum"] == "NP"
    assert list(written[0]) == list(KEYS)
