import json
from types import MappingProxyType

import pytest
from conftest import read

import oxedi
from oxedi import (
    KitBroken,
    KitDifference,
    KitDiffers,
    KitEnd,
    KitOutput,
    KitPassed,
    KitReport,
    Spec,
    SpecError,
    Valid,
    kit_files,
    run_kit,
)

PACKAGES = Spec.builtin_packages()
TOP = "oxedi/views/005010/835/X221A1"
(TOP_TEXT,) = [text for text in PACKAGES if json.loads(text)["package"]["id"] == TOP]
CLEAN = read("balanced_5010_sample.txt")


def manifest(package: str = TOP, cases: dict | None = None) -> str:
    if cases is None:
        cases = {"a": {"input": "a.835", "expect": {"diagnostics": "d.txt"}}}
    return json.dumps({"kit": {"format": 1, "package": package}, "cases": cases})


def stack() -> Valid:
    valid = oxedi.validate_stack(TOP_TEXT, PACKAGES)
    assert isinstance(valid, Valid)
    return valid


def test_a_clean_file_passes_its_diagnostics_case():
    result = run_kit(manifest(), stack(), {"a.835": CLEAN, "d.txt": b""})
    assert isinstance(result, KitReport)
    assert result.passed
    assert result.package == TOP
    assert str(result) == f'kit "{TOP}": 1 of 1 cases pass\ncase "a": passes'
    (case,) = result.cases
    assert case.name == "a"
    assert case.passed
    assert isinstance(case.outcome, KitPassed)
    assert repr(result) == f"KitReport(package='{TOP}', passed=True)"


def test_a_difference_is_reported_as_typed_values():
    result = run_kit(manifest(), stack(), {"a.835": CLEAN, "d.txt": b"x\n"})
    assert not result.passed
    (case,) = result.cases
    assert not case.passed
    outcome = case.outcome
    assert isinstance(outcome, KitDiffers)
    (difference,) = outcome.differences
    assert isinstance(difference, KitDifference)
    assert difference.output == KitOutput.DIAGNOSTICS
    assert difference.file == "d.txt"
    assert difference.line == 1
    assert difference.expected == "x"
    assert isinstance(difference.actual, KitEnd)
    assert str(difference) == 'diagnostics (d.txt) line 1: expected "x", found the end'
    assert str(case) == 'case "a": differs: diagnostics (d.txt) line 1: expected "x", found the end'
    assert repr(difference) == "KitDifference(output=KitOutput.DIAGNOSTICS, file='d.txt', line=1)"


def test_a_missing_file_breaks_the_case_naming_it():
    result = run_kit(manifest(), stack(), {"a.835": CLEAN})
    (case,) = result.cases
    assert not case.passed
    outcome = case.outcome
    assert isinstance(outcome, KitBroken)
    assert outcome.path == "d.txt"
    assert str(outcome) == outcome.message == 'the kit names "d.txt", which was not given; give the file to the runner'


def test_an_input_that_is_not_a_document_breaks_the_case():
    (case,) = run_kit(manifest(), stack(), {"a.835": b"not x12", "d.txt": b""}).cases
    assert isinstance(case.outcome, KitBroken)
    assert case.outcome.path == "a.835"
    assert case.outcome.message.startswith('"a.835" is not an X12 document: ')


def test_kit_files_lists_every_path_sorted():
    assert kit_files(manifest()) == ["a.835", "d.txt"]


def test_a_bad_manifest_raises_spec_error():
    with pytest.raises(SpecError) as caught:
        run_kit(manifest(cases={}), stack(), {})
    assert str(caught.value) == 'the kit is invalid: the kit has no case; add one under "cases"'
    with pytest.raises(SpecError, match="the kit is invalid"):
        kit_files(manifest(cases={}))


def test_a_kit_of_another_package_raises_spec_error():
    other = "oxedi/guide/005010/835/X221A1"
    with pytest.raises(SpecError) as caught:
        run_kit(manifest(package=other), stack(), {})
    assert str(caught.value) == (
        f'the kit checks package "{other}", but the stack\'s top package is "{TOP}"; run the kit '
        f'on the stack that has "{other}" on top, or change the kit\'s "package" to "{TOP}"'
    )


def test_files_hold_bytes():
    with pytest.raises(TypeError):
        run_kit(manifest(), stack(), {"a.835": "text", "d.txt": b""})  # type: ignore[dict-item]


def test_files_may_be_any_mapping():
    files = MappingProxyType({"a.835": CLEAN, "d.txt": b""})
    assert run_kit(manifest(), stack(), files).passed
