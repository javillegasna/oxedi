import inspect
import os
import shutil
import warnings

import edi_835_parser
import n104_shim
import pandas as pd
import pytest

from conftest import CORE_TESTS, SAMPLES, path_of
from oxedi835 import edi_835_parser as compat

n104_shim.apply()

SHAPES = {
    "edi835_test_davisvision.RMT": (2, 23),
    "edi835_test_eyemed.RMT": (414, 21),
    "edi835_test_file.RMT": (23, 21),
    "edi835_test_not_available_claim_id.RMT": (26, 23),
    "edi835_test_united.rmt": (6192, 26),
    "edi835_test_versant.RMT": (1778, 28),
}
# claims, patients, BPR02
COUNTS = {
    "edi835_test_davisvision.RMT": (1, 1, 0.0),
    "edi835_test_eyemed.RMT": (82, 82, 8982.0),
    "edi835_test_file.RMT": (4, 4, 8982.0),
    "edi835_test_not_available_claim_id.RMT": (18, 5, 3715.0),
    "edi835_test_united.rmt": (1332, 1212, 173305.0),
    "edi835_test_versant.RMT": (648, 417, 123950.65),
}


def old(path):
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        return edi_835_parser.parse(str(path))


def organization(org):
    return (
        org.organization.type,
        org.organization.name,
        org.organization.identification_code,
        org.address.address if org.address else None,
        (org.location.city, org.location.state, org.location.zip_code) if org.location else None,
    )


@pytest.fixture(params=SAMPLES)
def sample(request):
    return request.param


def by_path(path):
    return compat.parse(path)


def by_bytes(path):
    return compat.parse_bytes(path.read_bytes())


def by_file(path):
    with open(path, "rb") as handle:
        return compat.parse_file_obj(handle)


@pytest.mark.parametrize("route", [by_path, by_bytes, by_file])
def test_the_frame_equals_edi_835_parser_cell_for_cell(sample, route):
    expected = old(path_of(sample)).to_dataframe()
    actual = route(path_of(sample)).to_dataframe()
    pd.testing.assert_frame_equal(actual, expected, check_exact=True)
    assert actual.shape == SHAPES[sample]


def test_path_bytes_memoryview_and_file_give_the_same_frame(sample):
    path = path_of(sample)
    frames = [route(path).to_dataframe() for route in (by_path, by_bytes, by_file)]
    frames.append(compat.parse_bytes(memoryview(bytearray(path.read_bytes()))).to_dataframe())
    with open(path, "rb") as handle:
        frames.append(compat.parse_file_obj(handle, file_path="x.835").to_dataframe())
    for frame in frames[1:]:
        pd.testing.assert_frame_equal(frame, frames[0], check_exact=True)


def test_file_path_is_kept_or_defaults_to_bytes():
    data = path_of(SAMPLES[0]).read_bytes()
    assert [t.file_path for t in compat.parse_bytes(data)] == ["<bytes>"]
    assert [t.file_path for t in compat.parse_bytes(data, file_path="a.835")] == ["a.835"]
    assert [t.file_path for t in compat.parse(path_of(SAMPLES[0]))] == [str(path_of(SAMPLES[0]))]


def test_parse_many_equals_the_directory(tmp_path):
    for name in SAMPLES:
        shutil.copy(path_of(name), tmp_path / f"{name}.txt")
    order = [tmp_path / n for n in os.listdir(tmp_path)]
    expected = old(tmp_path).to_dataframe()
    with open(order[0], "rb") as first:
        actual = compat.parse_many([first, *(p.read_bytes() for p in order[1:])]).to_dataframe()
    pd.testing.assert_frame_equal(actual, expected, check_exact=True)


def test_counts_and_sums_equal_edi_835_parser(sample):
    expected, actual = old(path_of(sample)), compat.parse(path_of(sample))
    assert (actual.count_claims(), actual.count_patients(), actual.sum_payments()) == (
        expected.count_claims(), expected.count_patients(), expected.sum_payments())
    assert actual.count_claims() == COUNTS[sample][0]
    assert actual.count_patients() == COUNTS[sample][1]
    assert actual.sum_payments() == pytest.approx(COUNTS[sample][2], abs=0.005)


def test_payer_payee_and_segments_equal_edi_835_parser(sample):
    (expected,), (actual,) = list(old(path_of(sample))), list(compat.parse(path_of(sample)))
    assert organization(actual.payer) == organization(expected.payer)
    assert organization(actual.payee) == organization(expected.payee)
    for field in ("amount_paid", "payment_method", "routing_number", "transaction_date"):
        assert getattr(actual.financial_information, field) == getattr(expected.financial_information, field)
    for field in ("authorization_information_qualifier", "sender", "receiver", "transmission_date"):
        assert getattr(actual.interchange, field) == getattr(expected.interchange, field)


def test_claim_and_service_objects_equal_edi_835_parser(sample):
    (expected,), (actual,) = list(old(path_of(sample))), list(compat.parse(path_of(sample)))
    assert len(actual.claims) == len(expected.claims)
    for a, e in zip(actual.claims, expected.claims):
        assert (a.claim.marker, a.claim.icn, a.claim.charge_amount, a.claim.paid_amount, a.claim.claim_type) == (
            e.claim.marker, e.claim.icn, e.claim.charge_amount, e.claim.paid_amount, e.claim.claim_type)
        assert [(x.entity, x.type, x.name, x.identification_code_qualifier, x.identification_code)
                for x in a.entities] == [
            (x.entity, x.type, x.name, x.identification_code_qualifier, x.identification_code)
            for x in e.entities]
        assert (str(a.claim.status.payer_classification), a.claim.status.was_forwarded, a.claim.status.description) == (
            str(e.claim.status.payer_classification), e.claim.status.was_forwarded, e.claim.status.description)
        assert [(d.qualifier, d.date) for d in a.dates] == [(d.qualifier, d.date) for d in e.dates]
        assert [str(r) for r in a.references] == [str(r) for r in e.references]
        amounts = [(x.amount.qualifier, x.amount.amount) if x.amount else None for x in (a, e)]
        assert amounts[0] == amounts[1]
        assert len(a.services) == len(e.services)
        for s, t in zip(a.services, e.services):
            assert [(str(x.group_code), str(x.reason_code), x.amount) for x in s.adjustments] == [
                (str(x.group_code), str(x.reason_code), x.amount) for x in t.adjustments]
            assert [(d.qualifier, d.date) for d in s.dates] == [(d.qualifier, d.date) for d in t.dates]
            assert [(str(r.qualifier), str(r.code)) for r in s.remarks] == [(str(r.qualifier), str(r.code)) for r in t.remarks]
            assert [str(r) for r in s.references] == [str(r) for r in t.references]
            assert s.allowed_amount == t.allowed_amount


def test_a_directory_reads_the_same_files_in_the_same_order(tmp_path):
    for name in SAMPLES:
        shutil.copy(path_of(name), tmp_path / f"{name}.txt")
    (tmp_path / "ignored.RMT").write_bytes(path_of(SAMPLES[0]).read_bytes())
    expected, actual = old(tmp_path), compat.parse(tmp_path)
    assert len(actual) == len(expected) == 6
    pd.testing.assert_frame_equal(actual.to_dataframe(), expected.to_dataframe(), check_exact=True)
    assert actual.sum_payments() == expected.sum_payments()


def test_parse_has_the_library_signature():
    ours = inspect.signature(compat.parse).parameters
    theirs = inspect.signature(edi_835_parser.parse).parameters
    assert [(p.name, p.kind, p.default) for p in ours.values()] == [
        (p.name, p.kind, p.default) for p in theirs.values()]
    assert list(ours) == ["path", "debug"]


def test_parse_fails_on_bytes_as_the_library_does():
    with pytest.raises(FileNotFoundError) as expected:
        edi_835_parser.parse(b"ISA*00*")
    with pytest.raises(FileNotFoundError) as actual:
        compat.parse(b"ISA*00*")
    assert str(actual.value) == str(expected.value) == "[Errno 2] No such file or directory: b'ISA*00*'"


MIT_GRANT = "Permission is hereby granted, free of charge, to any person obtaining a copy"
LIBRARY_COPYRIGHT = "Copyright (c) 2018 The Python Packaging Authority"


def test_the_copied_code_tables_carry_the_library_license():
    from oxedi835.edi_835_parser import _codes

    header = _codes.__doc__
    assert "edi-835-parser" in header and "keiron-stoddart / Senscio Systems" in header
    assert LIBRARY_COPYRIGHT in header and MIT_GRANT in header
    notices = (CORE_TESTS.parents[2] / "THIRD_PARTY_NOTICES").read_text()
    assert "edi-835-parser 1.8.0" in notices and "keiron-stoddart / Senscio Systems" in notices
    assert LIBRARY_COPYRIGHT in notices and MIT_GRANT in notices
    assert "oxedi835/edi_835_parser/_codes.py" in notices
