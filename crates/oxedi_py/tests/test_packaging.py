import re
from importlib import metadata

import pytest

import oxedi


def test_the_base_package_requires_nothing():
    requirements = metadata.requires("oxedi") or []
    assert requirements and all("extra ==" in r for r in requirements)


def test_the_extras_are_the_documented_ones():
    assert sorted(metadata.metadata("oxedi").get_all("Provides-Extra")) == [
        "edi-835-parser", "pandas", "polars", "pyx12",
    ]


PEP_440 = re.compile(
    r"^([1-9][0-9]*!)?(0|[1-9][0-9]*)(\.(0|[1-9][0-9]*))*"
    r"((a|b|rc)(0|[1-9][0-9]*))?(\.post(0|[1-9][0-9]*))?(\.dev(0|[1-9][0-9]*))?"
    r"(\+[a-z0-9]+(\.[a-z0-9]+)*)?$"
)


def test_the_module_version_is_the_distribution_version():
    version = metadata.version("oxedi")
    assert oxedi.__version__ == version
    try:
        from packaging.version import Version
    except ImportError:
        assert PEP_440.match(version), version
    else:
        assert str(Version(version)) == version


ADAPTERS = ("pyx12", "edi_835_parser")


def _outside_the_public_api(source):
    """What an adapter takes from `oxedi` other than names `oxedi.__all__`
    holds: another module of `oxedi`, a name it does not export, any
    absolute `import oxedi`, and `_core` or a name it does not export read as
    an attribute of `oxedi` or of the name it is bound to."""
    import ast

    tree, bound, outside = ast.parse(source), set(), []
    for node in ast.walk(tree):
        name = getattr(node, "module", None) or ""
        if isinstance(node, ast.ImportFrom) and (node.level == 2 and not name or name == "oxedi"):
            outside += [a.name for a in node.names if a.name not in oxedi.__all__]
        elif isinstance(node, ast.ImportFrom) and (node.level == 2 or name.startswith("oxedi.")):
            outside.append(name)
        elif isinstance(node, ast.Import) and any(a.name.split(".")[0] == "oxedi" for a in node.names):
            outside += [a.name for a in node.names]
            bound |= {a.asname or "oxedi" for a in node.names}
    for node in ast.walk(tree):
        if isinstance(node, ast.Attribute) and (
            node.attr == "_core"
            or getattr(node.value, "id", None) in bound and node.attr not in oxedi.__all__
        ):
            outside.append(node.attr)
    return sorted(outside)


@pytest.mark.parametrize("source,outside", [
    ("from .. import Diagnostic, Spec\nfrom ._tables import load", []),
    ("from oxedi import Spec\nimport numpy as np", []),
    ("from .. import Diagnostic, _core", ["_core"]),
    ("from .._json import JsonObject", ["_json"]),
    ("from oxedi import _core", ["_core"]),
    ("from oxedi._core import parse", ["oxedi._core"]),
    ("import oxedi as o\no.Spec", ["oxedi"]),
    ("import oxedi as o\no._core.parse", ["_core", "oxedi"]),
    ("import oxedi\noxedi._private", ["_private", "oxedi"]),
])
def test_the_adapter_import_guard_catches_every_form(source, outside):
    assert _outside_the_public_api(source) == outside


def test_the_adapters_import_only_names_the_package_exports():
    from pathlib import Path

    root = Path(oxedi.__file__).parent
    files = [path for name in ADAPTERS for path in sorted((root / name).glob("*.py"))]
    assert files
    assert {p.name: f for p in files if (f := _outside_the_public_api(p.read_text()))} == {}
