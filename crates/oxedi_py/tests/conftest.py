import os
from pathlib import Path

import pytest

# The variable is set when the tests run from a copy outside the repository.
CORE_TESTS = Path(
    os.environ.get(
        "OXEDI_CORE_TESTS",
        Path(__file__).resolve().parents[2] / "oxedi_core" / "tests",
    )
)
GOLDEN = CORE_TESTS / "golden" / "project"
# The goldens of oxedi.pyx12.validate: next to the tests and not under
# CORE_TESTS, so they travel with the suite when it runs from a copy.
PYX12_GOLDEN = Path(__file__).resolve().parent / "golden" / "pyx12"

# How to rewrite the goldens of oxedi.pyx12.validate.
REGENERATE = 'make sandbox CMD="UPDATE_GOLDEN=1 make py-test"'

FIXTURES = [
    "emedny_sample.txt",
    "united_healthcare_legacy_sample.txt",
    "multi_claim_sample.txt",
    "trizetto_sample.rmt",
    "blue_cross_nc_sample.txt",
    "balanced_5010_sample.txt",
    "balanced_4010_sample.txt",
    "edge_cases_5010_sample.txt",
    "edge_cases_4010_sample.txt",
    "masked_findings_5010_sample.txt",
    "masked_findings_4010_sample.txt",
]
SAMPLES = [
    "edi835_test_davisvision.RMT",
    "edi835_test_eyemed.RMT",
    "edi835_test_file.RMT",
    "edi835_test_not_available_claim_id.RMT",
    "edi835_test_united.rmt",
    "edi835_test_versant.RMT",
]
ALL_FILES = FIXTURES + SAMPLES
SUMMARY_ONLY = {"edi835_test_united.rmt", "edi835_test_versant.RMT"}
NO_ISA = {"blue_cross_nc_sample.txt"}
LARGEST = "edi835_test_united.rmt"
# The files oxedi.pyx12.validate accepts.
VALIDATED = [name for name in ALL_FILES if name not in NO_ISA]


def path_of(name):
    folder = "fixtures" if name in FIXTURES else "samples"
    return CORE_TESTS / folder / name


def read(name):
    return path_of(name).read_bytes()


@pytest.fixture(params=ALL_FILES)
def file_name(request):
    return request.param


def delimiters_for(name):
    import oxedi

    return oxedi.Delimiters() if name in NO_ISA else None


def pyx12_golden(name):
    return PYX12_GOLDEN / f"{name}.json"


def parse_named(name):
    import oxedi

    return oxedi.parse(read(name), delimiters=delimiters_for(name))
