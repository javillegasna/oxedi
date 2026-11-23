import json

import pytest
from conftest import read

import oxedi
from oxedi import KitResult, Spec, SpecError, Valid, kit_files, run_kit

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
    assert isinstance(result, KitResult)
    assert result.passed
    assert result.package == TOP
    assert str(result) == f'kit "{TOP}": 1 of 1 cases pass\ncase "a": passes'
    (case,) = result.cases
    assert (case.name, case.passed, str(case)) == ("a", True, 'case "a": passes')
    assert repr(result) == f"KitResult(package='{TOP}', passed=True)"
    assert repr(case) == "KitCase(name='a', passed=True)"


def test_a_difference_is_reported_with_its_first_line():
    result = run_kit(manifest(), stack(), {"a.835": CLEAN, "d.txt": b"x\n"})
    assert not result.passed
    (case,) = result.cases
    assert not case.passed
    assert str(case) == 'case "a": differs: diagnostics (d.txt) line 1: expected "x", found the end'


def test_a_missing_file_breaks_the_case_naming_it():
    (case,) = run_kit(manifest(), stack(), {"a.835": CLEAN}).cases
    assert not case.passed
    assert str(case) == (
        'case "a": cannot run: the kit names "d.txt", which was not given; give the file to the runner'
    )


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
