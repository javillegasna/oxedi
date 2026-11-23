import ast
import inspect
import json
from collections import Counter

import pytest

import oxedi
from oxedi import Capability, Change, ChangeKind, Invalid, Spec, SpecError, Valid

PACKAGES = Spec.builtin_packages()


def package_text(package_id: str) -> str:
    """The shipped text whose header names `package_id`."""
    (found,) = [text for text in PACKAGES if json.loads(text)["package"]["id"] == package_id]
    return found


STACK_5010 = package_text("oxedi/views/005010/835/X221A1")
GUIDE_5010 = package_text("oxedi/guide/005010/835/X221A1")
STACK_4010 = package_text("oxedi/guide/004010/835/X091A1")

PATCH = json.dumps(
    {
        "package": {
            "format": 1,
            "id": "me/patch/005010/835/X221A1",
            "layers": ["patch"],
            "bases": ["oxedi/views/005010/835/X221A1"],
        },
        "spec": {"tables": {"payments": {"columns": {"extra": {"segment": "TRN", "element": 1}}}}},
    }
)


def test_the_5010_stack_is_valid():
    report = oxedi.validate_stack(STACK_5010, PACKAGES)
    assert isinstance(report, Valid)
    assert report.packages == [
        "oxedi/envelope/005010",
        "oxedi/dictionary/005010",
        "oxedi/transaction-set/005010/835",
        "oxedi/guide/005010/835/X221A1",
        "oxedi/views/005010/835/X221A1",
    ]
    assert report.id == "oxedi/views/005010/835/X221A1"
    assert report.name == "835"
    assert isinstance(report.write, Capability)
    assert report.write.can and report.mask.can
    assert report.write.reasons == [] and report.changes == []
    assert str(report) == (
        'yes: stack "oxedi/views/005010/835/X221A1" loads as spec "835"\n'
        'packages, bases first: "oxedi/envelope/005010" (narrows), '
        '"oxedi/dictionary/005010" (narrows), "oxedi/transaction-set/005010/835" (narrows), '
        '"oxedi/guide/005010/835/X221A1" (narrows), "oxedi/views/005010/835/X221A1" (narrows)\n'
        "can read\ncan write\ncan mask\nchanges: none"
    )
    assert repr(report) == "Valid(id='oxedi/views/005010/835/X221A1', name='835')"
    assert report.spec.tables() == Spec.builtin().tables()


def test_the_4010_stack_lists_81_changes():
    report = oxedi.validate_stack(STACK_4010, PACKAGES)
    assert isinstance(report, Valid)
    assert len(report.changes) == 81
    first = report.changes[0]
    assert isinstance(first, Change)
    assert first.package == "oxedi/envelope/004010"
    assert first.path == "segments.ISA.elements.11.type"
    assert first.kind == ChangeKind.CHANGED
    assert str(first) == (
        'segments.ISA.elements.11.type: "AN" becomes "ID" (package "oxedi/envelope/004010")'
    )
    assert repr(first) == (
        "Change(package='oxedi/envelope/004010', path='segments.ISA.elements.11.type', "
        "kind=ChangeKind.CHANGED)"
    )
    assert first == report.changes[0] and hash(first) == hash(report.changes[0])
    kinds = Counter(change.kind for change in report.changes)
    assert kinds == {
        ChangeKind.WIDENED: 30,
        ChangeKind.REMOVED: 24,
        ChangeKind.NARROWED: 13,
        ChangeKind.ADDED: 12,
        ChangeKind.CHANGED: 2,
    }


def test_a_guide_top_cannot_mask():
    report = oxedi.validate_stack(GUIDE_5010, PACKAGES)
    assert isinstance(report, Valid)
    assert report.mask.can is False
    assert report.mask.reasons == [
        'no column of any table is flagged "phi"; flag the columns that hold names, ids or '
        'dates with "phi": "name", "id" or "date"'
    ]
    assert repr(report.mask) == "Capability(can=False, reasons=1)"


def test_a_broken_top_is_invalid():
    report = oxedi.validate_stack("{")
    assert isinstance(report, Invalid)
    assert str(report) == (
        "no: the top package does not read: invalid spec JSON: "
        "EOF while parsing an object at line 1 column 1"
    )
    assert repr(report).startswith("Invalid('no: the top package does not read")


def test_a_broken_pool_text_names_its_position():
    report = oxedi.validate_stack(STACK_5010, ["{"])
    assert str(report).startswith("no: package 1 of the pool does not read: ")


def test_from_packages_loads_a_patch_package():
    spec = Spec.from_packages(PATCH, PACKAGES)
    assert spec.column_source("payments", "extra").segment == b"TRN"


def test_from_packages_raises_the_no_text():
    with pytest.raises(SpecError) as error:
        Spec.from_packages(PATCH, [])
    assert str(error.value) == str(oxedi.validate_stack(PATCH, []))


def test_pool_must_not_be_a_str():
    with pytest.raises(TypeError):
        oxedi.validate_stack(STACK_5010, "abc")


def test_an_invalid_repr_follows_the_rules_of_a_python_repr():
    quoted = json.loads(PATCH)
    quoted["package"]["it's"] = 1
    report = oxedi.validate_stack(json.dumps(quoted))
    assert isinstance(report, Invalid)
    assert "it's" in str(report)
    shown = ast.literal_eval(repr(report)[len("Invalid(") : -1])
    assert str(report).startswith(shown.removesuffix("..."))
    assert shown.endswith("...")


def test_the_default_pool_is_an_empty_tuple():
    assert str(inspect.signature(oxedi.validate_stack)) == "(top, pool=())"
    assert str(inspect.signature(Spec.from_packages)) == "(top, pool=())"
    assert isinstance(oxedi.validate_stack(STACK_5010, ()), Invalid)
