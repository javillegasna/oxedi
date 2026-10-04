"""The pyx12 subpackage without pyx12: its diagnostic type and the missing-extra error."""

import sys

import pytest

import oxedi835
from oxedi835.pyx12 import Pyx12Diagnostic, validate

from conftest import read


def test_a_finding_displays_origin_rule_code_place_and_datum():
    d = Pyx12Diagnostic(
        kind="Pyx12ElementError",
        rule='(NP) is not a valid code for Payee State Code (N402)',
        code="7",
        segment=15,
        span=(659, 682),
        element=2,
        component=3,
        datum=b'N"P',
    )
    assert str(d) == (
        'pyx12 · (NP) is not a valid code for Payee State Code (N402) (code 7)'
        ' · segment #15, bytes 659..682, element 2, component 3 · datum "N\\"P"'
    )
    assert (d.origin, d.level, d.path) == ("pyx12", None, "")


def test_a_failure_without_a_segment_displays_no_segment():
    d = Pyx12Diagnostic(kind="Pyx12Failure", rule="pyx12 could not finish validating: boom")
    assert str(d) == (
        "pyx12 · pyx12 could not finish validating: boom · no segment · datum \"\""
    )


def test_without_pyx12_validate_names_the_extra(monkeypatch):
    monkeypatch.setitem(sys.modules, "pyx12", None)
    with pytest.raises(ImportError) as info:
        validate(read("emedny_sample.txt"))
    assert str(info.value) == (
        "oxedi835.pyx12 needs the pyx12 package: pip install 'oxedi835[pyx12]'"
    )


def test_the_subpackage_imports_without_pyx12(monkeypatch):
    monkeypatch.setitem(sys.modules, "pyx12", None)
    import importlib

    module = importlib.reload(sys.modules["oxedi835.pyx12"])
    assert module.__all__ == ["Pyx12Diagnostic", "validate"]
    assert not hasattr(oxedi835, "validate")
