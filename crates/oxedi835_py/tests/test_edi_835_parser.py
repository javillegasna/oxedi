import datetime
import inspect
import os
import shutil
import warnings

import pytest

edi_835_parser = pytest.importorskip("edi_835_parser")
pd = pytest.importorskip("pandas")

import n104_shim  # noqa: E402
from conftest import CORE_TESTS, LARGEST, SAMPLES, path_of  # noqa: E402
from edi_835_parser.segments import organization as library_organization  # noqa: E402
from oxedi835 import ParseError  # noqa: E402
from oxedi835 import edi_835_parser as compat  # noqa: E402
from oxedi835.edi_835_parser import _codes  # noqa: E402
from oxedi835.edi_835_parser._convert import ENCODING  # noqa: E402

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
            assert (s.service.charge_amount, s.service.paid_amount) == (
                t.service.charge_amount, t.service.paid_amount)
            assert [(x.amount.qualifier, x.amount.amount) if x.amount else None for x in (s,)] == [
                (x.amount.qualifier, x.amount.amount) if x.amount else None for x in (t,)]


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
    header = _codes.__doc__
    assert "edi-835-parser" in header and "keiron-stoddart / Senscio Systems" in header
    assert LIBRARY_COPYRIGHT in header and MIT_GRANT in header
    notices = (CORE_TESTS.parents[2] / "THIRD_PARTY_NOTICES").read_text()
    assert "edi-835-parser 1.8.0" in notices and "keiron-stoddart / Senscio Systems" in notices
    assert LIBRARY_COPYRIGHT in notices and MIT_GRANT in notices
    assert "oxedi835/edi_835_parser/_codes.py" in notices
    assert "claim status registry" in notices and "organization names" in notices


def test_the_organization_names_and_statuses_equal_the_library():
    from edi_835_parser.elements import claim_status, organization as names

    assert _codes.ORGANIZATIONS == names.organizations
    assert _codes.STATUSES == {
        s.code: (s.description, str(s.payer_classification), s.was_forwarded)
        for s in claim_status._REGISTRY
    }


def synthetic(sender="SENDER", receiver="RECEIVER", payer=True, payee_name="CLINIC"):
    """A one-claim, one-service 835 built in the test."""
    isa = (f"ISA*00*          *00*          *ZZ*{sender:<15}*ZZ*{receiver:<15}"
           "*240101*1200*^*00501*000000001*0*P*:")
    segments = [isa, "GS*HP*S*R*20240101*1200*1*X*005010X221A1", "ST*835*0001",
                "BPR*I*100*C*ACH*CCP*01*999999999*DA*123456*1512345678**01*999999999*DA*123456*20240102",
                "TRN*1*12345*1512345678"]
    if payer:
        segments += ["N1*PR*ACME INSURANCE", "N3*1 MAIN ST", "N4*SPRINGFIELD*NY*12345"]
    segments += [f"N1*PE*{payee_name}*XX*1234567890", "LX*1", "CLP*C1*1*100*80**MC*ICN1",
                 "NM1*QC*1*DOE*JANE****MI*M1", "SVC*HC:99213*100*80**1", "DTM*472*20240101",
                 "CAS*CO*45*20", "SE*13*0001", "GE*1*1", "IEA*1*000000001"]
    return "~".join(segments).encode("latin-1") + b"~"


def recorded(call):
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        value = call()
    return value, [str(w.message) for w in caught
                   if str(w.message).startswith("Failed to build a transaction set")]


def test_a_synthetic_file_equals_the_library(tmp_path):
    path = tmp_path / "a.835"
    path.write_bytes(synthetic())
    pd.testing.assert_frame_equal(
        compat.parse(path).to_dataframe(), old(path).to_dataframe(), check_exact=True)


def two_transactions():
    """One interchange holding the transactions of ``synthetic(payee_name="FIRST")``
    and ``synthetic(payee_name="SECOND")``."""
    first, second = (synthetic(payee_name=name).decode("latin-1").split("~")[:-1] for name in ("FIRST", "SECOND"))
    body = [s.replace("ST*835*0001", "ST*835*0002").replace("SE*13*0001", "SE*13*0002") for s in second[2:-2]]
    segments = first[:-2] + body + ["GE*2*1", "IEA*1*000000001"]
    return "~".join(segments).encode("latin-1") + b"~"


def test_each_transaction_of_a_file_has_only_its_own_rows():
    sets = list(compat.parse_bytes(two_transactions()))
    assert [t.payee.organization.name for t in sets] == ["FIRST", "SECOND"]
    for transaction_set, name in zip(sets, ("FIRST", "SECOND")):
        (alone,) = list(compat.parse_bytes(synthetic(payee_name=name)))
        pd.testing.assert_frame_equal(transaction_set.to_dataframe(), alone.to_dataframe(), check_exact=True)
        pd.testing.assert_frame_equal(
            transaction_set.to_dataframe(extended=True).drop(columns="x_claim"),
            alone.to_dataframe(extended=True).drop(columns="x_claim"), check_exact=True)
        assert [c.claim.icn for c in transaction_set.claims] == ["ICN1"]
    assert list(compat.parse_bytes(two_transactions()).to_dataframe(extended=True).x_claim) == [0, 1]


def test_availity_and_zirmed_are_named_as_the_library_names_them(tmp_path):
    path = tmp_path / "a.835"
    path.write_bytes(synthetic(sender="AV09311993", receiver="ZIRMED"))
    (expected,), (actual,) = list(old(path)), list(compat.parse(path))
    assert (actual.interchange.sender, actual.interchange.receiver) == (
        expected.interchange.sender, expected.interchange.receiver) == ("Availity", "Zirmed")


def undecodable():
    try:
        b"\xe9".decode(ENCODING)
    except UnicodeDecodeError:
        return False
    return True


@pytest.mark.skipif(undecodable(), reason="the locale encoding reads every byte")
def test_a_byte_the_encoding_cannot_read_skips_that_file_as_the_library_does(tmp_path):
    (tmp_path / "good.txt").write_bytes(synthetic())
    (tmp_path / "bad.txt").write_bytes(synthetic(payee_name="CLINIC \xe9"))
    expected, expected_warnings = recorded(lambda: edi_835_parser.parse(str(tmp_path)))
    actual, actual_warnings = recorded(lambda: compat.parse(str(tmp_path)))
    assert len(actual) == len(expected) == 1
    assert actual_warnings == expected_warnings
    assert len(actual_warnings) == 1 and actual_warnings[0].startswith(
        f"Failed to build a transaction set from {tmp_path}/bad.txt with error: ")
    pd.testing.assert_frame_equal(actual.to_dataframe(), expected.to_dataframe(), check_exact=True)
    with pytest.raises(UnicodeDecodeError) as theirs:
        edi_835_parser.parse(str(tmp_path / "bad.txt"))
    with pytest.raises(UnicodeDecodeError) as ours:
        compat.parse(tmp_path / "bad.txt")
    assert str(ours.value) == str(theirs.value)
    with pytest.raises(UnicodeDecodeError):
        compat.parse_bytes(synthetic(payee_name="CLINIC \xe9"))


NOT_835 = {"junk.txt": b"hello, this is not an 835\n", "empty.835": b""}


def broken(transaction_sets):
    """What a transaction set built from a non-835 file looks like."""
    (t,) = list(transaction_sets)
    return (t.interchange, t.financial_information, t.claims, t.organizations,
            t.to_dataframe().shape)


@pytest.mark.parametrize("name", sorted(NOT_835))
def test_a_file_that_is_not_an_835_gives_a_broken_set_as_the_library_does(tmp_path, name):
    path = tmp_path / name
    path.write_bytes(NOT_835[name])
    expected, actual = old(path), compat.parse(path)
    assert broken(actual) == broken(expected) == (None, None, [], [], (0, 0))
    pd.testing.assert_frame_equal(actual.to_dataframe(), expected.to_dataframe(), check_exact=True)
    with pytest.raises(AttributeError) as theirs:
        expected.sum_payments()
    with pytest.raises(AttributeError) as ours:
        actual.sum_payments()
    assert str(ours.value) == str(theirs.value)
    with open(path, "rb") as handle:
        assert broken(compat.parse_file_obj(handle)) == broken(expected)
    assert broken(compat.parse_bytes(NOT_835[name])) == broken(expected)
    assert [broken([t]) for t in compat.parse_many([NOT_835[name], NOT_835[name]])] == [broken(expected)] * 2


@pytest.mark.parametrize("debug", [False, True])
def test_a_directory_keeps_files_that_are_not_835_as_the_library_does(tmp_path, debug):
    shutil.copy(path_of(SAMPLES[0]), tmp_path / "sample.txt")
    for name, data in NOT_835.items():
        (tmp_path / name).write_bytes(data)
    expected, expected_warnings = recorded(lambda: edi_835_parser.parse(str(tmp_path), debug=debug))
    actual, actual_warnings = recorded(lambda: compat.parse(str(tmp_path), debug=debug))
    assert len(actual) == len(expected) == 3
    assert actual_warnings == expected_warnings == []
    assert [t.file_path for t in actual] == [t.file_path for t in expected]
    pd.testing.assert_frame_equal(actual.to_dataframe(), expected.to_dataframe(), check_exact=True)


def test_a_numeric_n104_is_an_int_as_in_the_library_without_the_shim(monkeypatch):
    monkeypatch.setattr(library_organization.Organization, "__init__", n104_shim.ORIGINAL)
    (expected,), (actual,) = list(old(path_of(LARGEST))), list(compat.parse(path_of(LARGEST)))
    assert type(expected.payee.organization.identification_code) is int
    assert type(actual.payee.organization.identification_code) is int
    assert organization(actual.payer) == organization(expected.payer)
    assert organization(actual.payee) == organization(expected.payee)


def test_a_transaction_without_a_payer_loop_names_the_input_and_segment():
    (transaction_set,) = list(compat.parse_bytes(synthetic(payer=False)))
    with pytest.raises(ValueError) as info:
        transaction_set.payer
    assert str(info.value) == "<bytes>: the transaction at segment 2 has no payer loop (N1)"


def test_a_file_without_a_transaction_says_it_has_no_payee_loop():
    (transaction_set,) = list(compat.parse_bytes(NOT_835["junk.txt"], file_path="junk.txt"))
    with pytest.raises(ValueError) as info:
        transaction_set.payee
    assert str(info.value) == "junk.txt: the file has no transaction (ST), so no payee loop (N1)"


def test_parse_accepts_a_path_like_and_fails_on_an_empty_path(tmp_path):
    path = tmp_path / "a.835"
    path.write_bytes(synthetic())
    pd.testing.assert_frame_equal(
        compat.parse(path).to_dataframe(), compat.parse(str(path)).to_dataframe(), check_exact=True)
    with pytest.raises(IndexError):
        edi_835_parser.parse("")
    with pytest.raises(FileNotFoundError):
        compat.parse("")


def test_an_unreadable_isa_raises_a_parse_error_naming_the_file(tmp_path):
    path = tmp_path / "short.835"
    path.write_bytes(b"ISA*00*~")
    with pytest.raises(ParseError) as info:
        compat.parse(path)
    assert str(info.value) == (
        f"{path}: ISA segment truncated after 8 bytes: found 2 of 16 element separators")


def test_only_the_compat_layer_reads_input_without_an_isa_as_the_library_does():
    import oxedi835

    with pytest.raises(ParseError) as info:
        oxedi835.parse(b"junk")
    assert str(info.value) == "input does not start with an ISA segment (found bytes [6a 75 6e 6b])"
    assert broken(compat.parse_bytes(b"junk")) == (None, None, [], [], (0, 0))


BOM = b"\xef\xbb\xbf"


def fields(transaction_sets):
    """The parts of each set a byte order mark can change in the library."""
    return [(t.interchange and (t.interchange.sender, t.interchange.receiver, t.interchange.transmission_date),
             t.financial_information.amount_paid,
             t.financial_information.transaction_date, len(t.claims)) for t in transaction_sets]


def test_a_byte_order_mark_loses_only_the_interchange_as_in_the_library(tmp_path):
    path = tmp_path / "bom.835"
    path.write_bytes(BOM + synthetic())
    expected = old(path)
    assert fields(expected) == [(None, 100.0, datetime.datetime(2024, 1, 2), 1)]
    with open(path, "rb") as handle:
        routes = [compat.parse(path), compat.parse_bytes(BOM + synthetic()), compat.parse_file_obj(handle)]
    for actual in routes:
        assert fields(actual) == fields(expected)
        pd.testing.assert_frame_equal(actual.to_dataframe(), expected.to_dataframe(), check_exact=True)
    plain = tmp_path / "plain"
    plain.mkdir()
    (plain / "a.835").write_bytes(synthetic())
    pd.testing.assert_frame_equal(
        compat.parse(path).to_dataframe(), old(plain / "a.835").to_dataframe(), check_exact=True)
    (tmp_path / "other.txt").write_bytes(synthetic(payee_name="OTHER CLINIC"))
    expected_dir, actual_dir = old(tmp_path), compat.parse(tmp_path)
    assert fields(actual_dir) == fields(expected_dir)
    pd.testing.assert_frame_equal(actual_dir.to_dataframe(), expected_dir.to_dataframe(), check_exact=True)


def test_the_native_parse_still_rejects_a_byte_order_mark():
    import oxedi835

    with pytest.raises(ParseError):
        oxedi835.parse(BOM + synthetic())


PREFIXES = [b"   \r\n", b"\n", b"\t\t", BOM, BOM + b"\n", BOM + b"  \r\n"]


@pytest.mark.parametrize("prefix", PREFIXES, ids=repr)
def test_a_prefix_before_the_isa_reads_as_in_the_library(tmp_path, prefix):
    data = prefix + path_of(SAMPLES[0]).read_bytes()
    path = tmp_path / "prefixed.835"
    path.write_bytes(data)
    expected = old(path)
    for actual in (compat.parse(path), compat.parse_bytes(data)):
        pd.testing.assert_frame_equal(actual.to_dataframe(), expected.to_dataframe(), check_exact=True)
        assert [t.interchange is None for t in actual] == [t.interchange is None for t in expected]


MARKED = [
    (BOM + b"\x0b", True),
    (BOM + b"\x0c", True),
    (BOM, False),
    (BOM + b"\n", False),
]


@pytest.mark.parametrize("prefix, sample", MARKED, ids=repr)
def test_a_byte_order_mark_never_raises_and_reads_as_in_the_library(tmp_path, prefix, sample):
    data = prefix + (path_of(SAMPLES[0]).read_bytes() if sample else b"ISA*bad~")
    path = tmp_path / "marked.835"
    path.write_bytes(data)
    expected = old(path)
    assert [t.interchange for t in expected] == [None]
    for actual in (compat.parse(path), compat.parse_bytes(data)):
        pd.testing.assert_frame_equal(actual.to_dataframe(), expected.to_dataframe(), check_exact=True)
        assert [t.interchange for t in actual] == [None]



# claims without services, claim adjustment groups, PLB groups, claim REF, claim AMT
RECOVERED = {
    "edi835_test_davisvision.RMT": (0, 0, 1, 2, 1),
    "edi835_test_eyemed.RMT": (0, 0, 0, 0, 81),
    "edi835_test_file.RMT": (0, 0, 0, 0, 3),
    "edi835_test_not_available_claim_id.RMT": (0, 0, 0, 36, 18),
    "edi835_test_united.rmt": (0, 0, 0, 40, 1247),
    "edi835_test_versant.RMT": (0, 3, 3, 1296, 643),
}


def cells(frame, prefix, suffix):
    columns = [c for c in frame.columns if c.startswith(prefix) and c.endswith(suffix)]
    return int(frame[columns].notna().sum().sum())


@pytest.mark.filterwarnings("ignore:Mismatched null-like:FutureWarning")
def test_extended_keeps_the_frame_and_adds_only_x_columns(sample):
    strict = compat.parse(path_of(sample)).to_dataframe()
    extended = compat.parse(path_of(sample)).to_dataframe(extended=True)
    width = len(strict.columns)
    assert list(extended.columns[:width]) == list(strict.columns)
    assert all(c.startswith("x_") for c in extended.columns[width:])
    services = extended[extended.x_row_kind == "service"]
    pd.testing.assert_frame_equal(
        services[list(strict.columns)].reset_index(drop=True),
        strict.reset_index(drop=True),
        check_dtype=False,
    )


def test_extended_recovers_what_the_frame_leaves_out(sample):
    extended = compat.parse(path_of(sample)).to_dataframe(extended=True)
    per_claim = extended[extended.x_row_kind != "provider_adjustment"].drop_duplicates("x_claim")
    recovered = (
        int((extended.x_row_kind == "claim").sum()),
        cells(per_claim, "x_claim_adj_", "_code"),
        int((extended.x_row_kind == "provider_adjustment").sum()),
        cells(per_claim, "x_claim_ref_", "_qual"),
        cells(per_claim, "x_claim_amt_", "_qual"),
    )
    assert recovered == RECOVERED[sample]


SERVICELESS = (
    "ISA*00*          *00*          *ZZ*SENDER         *ZZ*RECEIVER       *240103*0501*U*00401*000000001*0*P*:~"
    "GS*HP*SENDER*RECEIVER*20240103*0501*1*X*004010X091A1~"
    "ST*835*0001~"
    "BPR*I*100*C*ACH*CCP*01*999999999*DA*1*1234567890**01*999999999*DA*2*20240103~"
    "TRN*1*12345*1234567890~"
    "N1*PR*PAYER~N3*1 MAIN ST~N4*TOWN*ST*12345~"
    "N1*PE*PAYEE*XX*1234567893~"
    "LX*1~"
    "CLP*A1*1*150*100**12*X1~NM1*QC*1*DOE*JANE****MI*M1~"
    "SVC*HC:99213*150*100**1~DTM*472*20240101~CAS*CO*45*50~"
    "CLP*A2*4*80*0**12*X2~NM1*QC*1*ROE*RICK****MI*M2~CAS*CO*29*80~DTM*232*20240102~"
    "SE*17*0001~GE*1*1~IEA*1*000000001~"
)


def test_a_claim_without_services_is_left_out_and_recovered(tmp_path):
    path = tmp_path / "serviceless.txt"
    path.write_text(SERVICELESS)
    strict = compat.parse(path).to_dataframe()
    pd.testing.assert_frame_equal(strict, old(path).to_dataframe(), check_exact=True)
    assert compat.parse(path).count_claims() == old(path).count_claims() == 2
    extended = compat.parse(path).to_dataframe(extended=True)
    assert list(extended.x_row_kind) == ["service", "claim"]
    claim = extended.iloc[1]
    assert (claim.marker, claim.x_claim_adj_0_group, claim.x_claim_adj_0_code, claim.x_claim_adj_0_amount) == (
        "A2", "CO", "29", 80.0)
    assert pd.isna(claim.code) and claim.start_date == pd.Timestamp("2024-01-02")


def test_cell_types_equal_edi_835_parser(sample):
    expected_sets, actual_sets = old(path_of(sample)), compat.parse(path_of(sample))
    expected, actual = expected_sets.to_dataframe(), actual_sets.to_dataframe()
    assert list(actual.columns) == list(expected.columns)
    for column in expected.columns:
        assert actual[column].dtype == expected[column].dtype, column
        if expected[column].dtype == object:
            assert [type(v) for v in actual[column]] == [type(v) for v in expected[column]], column
    for name, kind in (("count_claims", int), ("count_patients", int), ("sum_payments", float)):
        assert type(getattr(actual_sets, name)()) is type(getattr(expected_sets, name)()) is kind, name


def plb_only():
    return (
        SERVICELESS.split("LX*1~")[0]
        + "PLB*1234567893*20231231*L6:ABC*-5.00~SE*9*0001~GE*1*1~IEA*1*000000001~"
    ).encode()


@pytest.mark.parametrize("plb_first", [True, False])
def test_extended_keeps_the_strict_columns_first_whichever_file_comes_first(tmp_path, plb_first):
    plb, normal = plb_only(), path_of(SAMPLES[2]).read_bytes()
    items = [plb, normal] if plb_first else [normal, plb]
    strict = compat.parse_many(items).to_dataframe()
    extended = compat.parse_many(items).to_dataframe(extended=True)
    width = len(strict.columns)
    assert list(extended.columns[:width]) == list(strict.columns)
    assert all(c.startswith("x_") for c in extended.columns[width:])
    assert (extended.x_row_kind == "provider_adjustment").sum() == 1


def test_the_extended_column_order_is_the_library_s():
    from oxedi835.edi_835_parser._extended import LIBRARY_COLUMNS

    strict = compat.parse(path_of(SAMPLES[0])).to_dataframe()
    assert list(LIBRARY_COLUMNS) == [c for c in strict.columns if not c.startswith(("adj_", "ref_", "rem_"))]


def test_trailing_blanks_before_the_terminator_are_stripped_as_the_library_does(tmp_path):
    import re

    text = path_of(SAMPLES[2]).read_bytes().decode()
    for segment in ("CLP", "DTM", "AMT", "N4", "NM1", "BPR", "CAS", "SVC", "REF", "TRN", "N1"):
        text = re.sub(rf"({segment}\*[^~]*)~", r"\1  ~", text)
    path = tmp_path / "blanks.txt"
    path.write_text(text)
    assert "  ~" in text
    expected, actual = old(path), compat.parse(path)
    pd.testing.assert_frame_equal(actual.to_dataframe(), expected.to_dataframe(), check_exact=True)
    (e,), (a,) = list(expected), list(actual)
    assert organization(a.payer) == organization(e.payer)
    assert organization(a.payee) == organization(e.payee)
    fields = ("amount_paid", "payment_method", "routing_number", "transaction_date")
    assert [getattr(a.financial_information, f) for f in fields] == [
        getattr(e.financial_information, f) for f in fields]
    assert [c.claim.icn for c in a.claims] == [c.claim.icn for c in e.claims]
    assert all(not c.claim.icn.endswith(" ") for c in a.claims)
