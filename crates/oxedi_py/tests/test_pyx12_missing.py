"""The pyx12 subpackage without pyx12: it imports, and validating names the extra."""

import sys

import pytest

import oxedi
from oxedi.pyx12 import validate

from conftest import read


def test_without_pyx12_validate_names_the_extra(monkeypatch):
    monkeypatch.setitem(sys.modules, "pyx12", None)
    with pytest.raises(ImportError) as info:
        validate(read("emedny_sample.txt"))
    assert str(info.value) == (
        "oxedi.pyx12 needs the pyx12 package: pip install 'oxedi[pyx12]'"
    )


def test_the_subpackage_imports_without_pyx12(monkeypatch):
    monkeypatch.setitem(sys.modules, "pyx12", None)
    import importlib

    module = importlib.reload(sys.modules["oxedi.pyx12"])
    assert module.__all__ == ["validate"]
    assert not hasattr(module, "Pyx12Diagnostic")
    assert not hasattr(oxedi, "validate")
