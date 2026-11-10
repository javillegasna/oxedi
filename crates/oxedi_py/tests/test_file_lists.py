"""The lists of fixtures and samples hold every file of their directories."""

import ast
import pathlib

from conftest import ALL_FILES, CORE_TESTS, FIXTURES, PYX12_GOLDEN, SAMPLES, VALIDATED


def test_every_fixture_and_sample_is_in_the_lists():
    """A file in the fixtures or samples directory that the lists leave out
    would be skipped by every suite that iterates them."""
    for folder, listed in (("fixtures", FIXTURES), ("samples", SAMPLES)):
        on_disk = {p.name for p in (CORE_TESTS / folder).iterdir() if p.name != "README.md"}
        missing = sorted(on_disk - set(listed))
        stale = sorted(set(listed) - on_disk)
        assert not missing and not stale, (
            f"tests/{folder}: files missing from the list in conftest.py: {missing}; "
            f"listed but not on disk: {stale}"
        )


def _literal_names(path, variable):
    """The strings that name files in the module-level literal `variable` of the
    test module `path`: the elements of a list or set, the keys of a dict.
    Read from the source, so a module that skips without its dependencies is
    still checked."""
    tree = ast.parse((pathlib.Path(__file__).parent / path).read_text())
    for node in tree.body:
        if isinstance(node, ast.Assign) and any(
            isinstance(t, ast.Name) and t.id == variable for t in node.targets
        ):
            value = node.value
            items = value.keys if isinstance(value, ast.Dict) else value.elts
            return {ast.literal_eval(item) for item in items}
    raise AssertionError(f"{path} has no module-level {variable}")


def test_the_per_sample_tables_name_every_sample():
    """SHAPES and COUNTS in test_edi_835_parser.py hold per-sample numbers, so
    they cannot be derived; they must name each sample."""
    for variable in ("SHAPES", "COUNTS"):
        named = _literal_names("test_edi_835_parser.py", variable)
        assert named == set(SAMPLES), (
            f"test_edi_835_parser.py {variable} vs conftest.py SAMPLES: "
            f"missing {sorted(set(SAMPLES) - named)}, unknown {sorted(named - set(SAMPLES))}"
        )


# The files test_write.py leaves to other suites: the ISA-less fragment has no
# envelope to write back.
NOT_WRITTEN = {"blue_cross_nc_sample.txt"}


def test_the_write_lists_name_every_file_but_the_stated_exceptions():
    clean = _literal_names("test_write.py", "CLEAN")
    refused = _literal_names("test_write.py", "REFUSED")
    assert not clean & refused, (
        f"in both CLEAN and REFUSED of test_write.py: {sorted(clean & refused)}"
    )
    expected = set(ALL_FILES) - NOT_WRITTEN
    named = clean | refused
    assert named == expected, (
        f"test_write.py CLEAN + REFUSED vs conftest.py files: "
        f"missing {sorted(expected - named)}, unknown {sorted(named - expected)}"
    )


def test_every_validated_file_has_a_pyx12_golden_and_nothing_else_does():
    on_disk = {p.name.removesuffix(".json") for p in PYX12_GOLDEN.iterdir()}
    expected = set(VALIDATED)
    missing = sorted(expected - on_disk)
    stale = sorted(on_disk - expected)
    assert not missing and not stale, (
        f"tests/golden/pyx12: goldens missing for {missing} "
        f'(run make sandbox CMD="UPDATE_GOLDEN=1 make py-test"); '
        f"goldens for no listed file: {stale}"
    )
