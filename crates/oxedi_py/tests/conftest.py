import os
from pathlib import Path

import pytest

# The variable is set when the tests run from a copy outside the repository.
CORE_TESTS = Path(
    os.environ.get(
        "OXEDI835_CORE_TESTS",
        Path(__file__).resolve().parents[2] / "oxedi_core" / "tests",
    )
)
GOLDEN = CORE_TESTS / "golden" / "project"

FIXTURES = [
    "emedny_sample.txt",
    "united_healthcare_legacy_sample.txt",
    "multi_claim_sample.txt",
    "trizetto_sample.rmt",
    "blue_cross_nc_sample.txt",
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


def parse_named(name):
    import oxedi

    return oxedi.parse(read(name), delimiters=delimiters_for(name))
