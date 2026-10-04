import sys
import warnings
from decimal import Decimal

import pytest

import oxedi835
from oxedi835 import Spec

edi_835_parser = pytest.importorskip("edi_835_parser")
pl = pytest.importorskip("polars")

import n104_shim  # noqa: E402
from conftest import LARGEST, SAMPLES, parse_named, path_of, read  # noqa: E402

n104_shim.apply()


def old(name):
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        return edi_835_parser.parse(str(path_of(name)))


@pytest.mark.parametrize("sample", SAMPLES)
def test_native_counts_equal_edi_835_parser(sample):
    result = parse_named(sample)
    expected = old(sample)
    assert result.count_claims() == expected.count_claims()
    assert result.count_patients() == expected.count_patients()
    assert isinstance(result.sum_payments(), Decimal)
    assert abs(float(result.sum_payments()) - expected.sum_payments()) < 0.005


def test_the_largest_sum_keeps_the_column_scale():
    assert str(parse_named(LARGEST).sum_payments()) == "173305.00"


def text(value):
    return None if value is None else str(value)


@pytest.mark.parametrize("sample", SAMPLES)
def test_native_payer_and_payee_equal_edi_835_parser(sample):
    result = parse_named(sample)
    (transaction,) = list(old(sample))
    for role in ("payer", "payee"):
        organization = getattr(transaction, role)
        location = organization.location
        assert getattr(result, role) == {
            "name": organization.organization.name,
            "identification_code": text(organization.organization.identification_code),
            "address": organization.address.address if organization.address else None,
            "city": location.city if location else None,
            "state": location.state if location else None,
            "zip_code": location.zip_code if location else None,
        }


def test_a_file_without_a_payee_loop_has_none():
    data = read("edi835_test_file.RMT")
    result = oxedi835.parse(data.replace(b"N1*PE*", b"N1*XX*"))
    assert result.payee is None
    assert result.payer is not None


def test_a_missing_table_is_named():
    spec = Spec.builtin().patch({"tables": {"payments": None}})
    result = oxedi835.parse(read("edi835_test_file.RMT"), spec=spec)
    with pytest.raises(KeyError) as info:
        result.sum_payments()
    assert info.value.args[0] == (
        'Result.sum_payments reads the table "payments"; the tables are: '
        "adjustments, claims, provider_adjustments, services"
    )


def test_a_missing_column_is_named():
    spec = Spec.builtin().patch({"tables": {"claims": {"columns": {"patient_id": None}}}})
    result = oxedi835.parse(read("edi835_test_file.RMT"), spec=spec)
    with pytest.raises(KeyError) as info:
        result.count_patients()
    assert info.value.args[0] == (
        'Result.count_patients reads the column "patient_id" of the table "claims"; its columns are: '
        + ", ".join(result.tables["claims"].columns)
    )


def test_a_non_decimal_amount_is_named():
    spec = Spec.builtin().patch({"segments": {"BPR": {"elements": {"2": {
        "name": "total_actual_provider_payment_amount", "type": "AN", "min": 1, "max": 18}}}}})
    result = oxedi835.parse(read("edi835_test_file.RMT"), spec=spec)
    with pytest.raises(TypeError) as info:
        result.sum_payments()
    assert str(info.value) == (
        'Result.sum_payments reads "total_payment_amount" of the table "payments" as a decimal; it is binary'
    )


def test_payer_reads_one_transaction():
    data = read("edi835_test_file.RMT")
    with pytest.raises(ValueError) as info:
        oxedi835.parse(data + data).payer
    assert str(info.value) == 'Result.payer reads one transaction; the table "payments" has 2 rows'


def test_payee_reads_one_transaction():
    data = read("edi835_test_file.RMT")
    with pytest.raises(ValueError) as info:
        oxedi835.parse(data + data).payee
    assert str(info.value) == 'Result.payee reads one transaction; the table "payments" has 2 rows'


def test_a_table_reaches_polars_and_pandas():
    pytest.importorskip("pandas")
    tables = parse_named(LARGEST).tables
    frame = tables["claims"].to_polars()
    assert frame.height == 1332
    assert frame.schema["charge_amount"] == pl.Decimal(38, 2)
    assert tables["claims"].to_pandas().shape == (1332, 24)
    frames = tables.to_polars()
    assert list(frames) == tables.keys()
    assert frames["services"].height == 6192
    assert list(tables.to_pandas()) == tables.keys()


@pytest.mark.parametrize("method, module, extra", [
    ("to_polars", "polars", "polars"),
    ("to_pandas", "pandas", "pandas"),
    ("to_pandas", "pyarrow", "pandas"),
])
@pytest.mark.parametrize("owner", ["Table", "Tables"])
def test_a_missing_extra_is_named(monkeypatch, owner, method, module, extra):
    tables = parse_named("edi835_test_file.RMT").tables
    target = tables["claims"] if owner == "Table" else tables
    monkeypatch.setitem(sys.modules, module, None)
    with pytest.raises(ImportError) as info:
        getattr(target, method)()
    assert str(info.value) == (
        f'{owner}.{method} needs {module}, which is not installed; '
        f'install it with: pip install "oxedi835[{extra}]"'
    )
    assert isinstance(info.value.__cause__, ImportError)


def test_a_sum_that_overflows_names_the_table_row_and_amount():
    data = read("edi835_test_file.RMT").decode()
    nines = "9" * 36
    data = data.replace("BPR*I*8982*", f"BPR*I*{nines}*").encode()
    spec = Spec.builtin().patch({"segments": {"BPR": {"elements": {"2": {
        "name": "total_actual_provider_payment_amount", "type": "R", "min": 1, "max": 40}}}}})
    result = oxedi835.parse(data + data, spec=spec)
    with pytest.raises(ValueError) as info:
        result.sum_payments()
    assert str(info.value) == (
        'Result.sum_payments: the sum of "total_payment_amount" in the table "payments" '
        f"overflows a 128-bit decimal at row 1 (amount {nines}.00)"
    )
