"""``oxedi.write``: tables back into a file, read back to the same tables."""

import datetime

import pyarrow
import pytest

import oxedi
from conftest import parse_named

# The files that read without a diagnostic, and so write strictly.
CLEAN = [
    "edi835_test_davisvision.RMT",
    "edi835_test_eyemed.RMT",
    "edi835_test_united.rmt",
    "edi835_test_versant.RMT",
    "emedny_sample.txt",
    "united_healthcare_legacy_sample.txt",
]


def envelope(**changes):
    fields = dict(
        sender_id="SENDER",
        receiver_id="RECEIVER",
        date=datetime.date(2024, 1, 1),
        time=datetime.time(12, 30),
    )
    fields.update(changes)
    return oxedi.Envelope(**fields)


def arrow(tables):
    """Every table as a pyarrow table, without the anchor segment's index
    (the written file holds fewer segments)."""
    out = {}
    for name in tables.keys():
        table = pyarrow.table(tables[name])
        out[name] = table.drop_columns(["segment"])
    return out


def assert_same(before, after):
    first, second = arrow(before), arrow(after)
    assert first.keys() == second.keys()
    for name in first:
        assert first[name].equals(second[name]), name


@pytest.mark.parametrize("name", CLEAN)
def test_the_tables_of_a_parse_write_back_to_the_same_tables(name):
    result = parse_named(name)
    data = oxedi.write(result.tables, envelope())
    again = oxedi.parse(data)
    assert again.diagnostics == []
    assert_same(result.tables, again.tables)


@pytest.mark.parametrize("kind", ["pyarrow", "polars", "pandas"])
def test_a_mapping_of_frames_writes_like_the_tables(kind):
    result = parse_named("emedny_sample.txt")
    tables = result.tables
    if kind == "pyarrow":
        frames = {name: pyarrow.table(tables[name]) for name in tables.keys()}
    elif kind == "polars":
        frames = {name: tables[name].to_polars() for name in tables.keys()}
    else:
        frames = {name: tables[name].to_pandas() for name in tables.keys()}
    data = oxedi.write(frames, envelope())
    assert data == oxedi.write(tables, envelope())


def test_the_spec_that_parsed_the_tables_writes_them():
    result = parse_named("edi835_test_davisvision.RMT")
    data = oxedi.write(result.tables, envelope())
    assert b"*00401*" in data
    assert b"~GS*HP*SENDER*RECEIVER*20240101*1230*1*X*004010X091A1~" in data
    frames = {name: result.tables[name].to_polars() for name in result.tables.keys()}
    assert oxedi.write(frames, envelope(), spec=oxedi.Spec.builtin("4010")) == data


def test_the_batches_of_a_stream_write_each_transaction():
    data = parse_named("emedny_sample.txt").document
    batches = list(oxedi.stream(bytes(b"".join(s.raw for s in data))))
    written = oxedi.write(batches[0].tables, envelope())
    assert written.count(b"~ST*835*") == 1


def test_the_envelope_holds_its_fields():
    given = envelope(control_number=7, usage_indicator="T", line_break=True)
    assert given.sender_id == "SENDER"
    assert given.sender_qualifier == "ZZ"
    assert given.receiver_id == "RECEIVER"
    assert given.receiver_qualifier == "ZZ"
    assert given.usage_indicator == "T"
    assert given.control_number == 7
    assert given.delimiters == oxedi.Delimiters(repetition=b"^")
    assert repr(given) == 'Envelope(sender_id="SENDER", receiver_id="RECEIVER", control_number=7)'
    data = oxedi.write(parse_named("emedny_sample.txt").tables, given)
    first = data.split(b"\n")[0]
    assert first == (
        b"ISA*00*          *00*          *ZZ*SENDER         *ZZ*RECEIVER       "
        b"*240101*1230*^*00501*000000007*0*T*:~"
    )
    with pytest.raises(ValueError, match="fraction of a second"):
        envelope(time=datetime.time(12, 30, 0, 5))


def test_a_finding_raises_with_every_finding_and_allow_findings_returns_them():
    result = parse_named("emedny_sample.txt")
    payments = result.tables["payments"].to_polars()
    import polars

    off = payments.with_columns(
        polars.lit(1, dtype=polars.Decimal(38, 2)).alias("total_payment_amount")
    )
    frames = {name: result.tables[name].to_polars() for name in result.tables.keys()}
    frames["payments"] = off
    with pytest.raises(oxedi.WriteError) as info:
        oxedi.write(frames, envelope())
    error = info.value
    assert isinstance(error, ValueError)
    assert str(error).startswith(
        "the tables do not make a valid file (1 finding); nothing was written\n"
        '1. table "payments" row 0 column "total_payment_amount": SNIP 3 · balancing rule'
    )
    [finding] = error.findings
    assert finding.kind == "ReadBack"
    assert (finding.table, finding.row, finding.column) == ("payments", 0, "total_payment_amount")
    assert finding.field is None
    assert finding.diagnostic.kind == "BalanceMismatch"
    assert finding.message == str(finding)
    assert repr(finding).startswith("WriteFinding(")
    data, findings = oxedi.write(frames, envelope(), allow_findings=True)
    assert [str(f) for f in findings] == [str(finding)]
    assert b"~BPR*I*1*C*" in data


def test_an_envelope_finding_names_its_field():
    tables = parse_named("emedny_sample.txt").tables
    data, findings = oxedi.write(tables, envelope(sender_id="SEND~ER"), allow_findings=True)
    assert [f.field for f in findings] == ["sender_id", "application_sender"]
    assert findings[0].kind == "DelimiterInValue"
    assert findings[0].table is None
    assert data.startswith(b"ISA*")


def test_tables_that_do_not_fit_the_spec_are_refused():
    tables = parse_named("emedny_sample.txt").tables
    with pytest.raises(oxedi.WriteError) as info:
        oxedi.write({"notes": pyarrow.table({"a": [1]})}, envelope())
    assert str(info.value) == (
        'table "notes" is not a table of the spec, whose tables are '
        '"adjustments", "claims", "payments", "provider_adjustments", "services"'
    )
    assert info.value.findings == []
    with pytest.raises(oxedi.WriteError, match='has no column "color" in the spec'):
        oxedi.write({"claims": pyarrow.table({"color": ["red"]})}, envelope())
    with pytest.raises(oxedi.WriteError, match=r"column \"charge_amount\" row 0: an Arrow Float64"):
        oxedi.write(
            {"claims": pyarrow.table({"charge_amount": pyarrow.array([1.234])})},
            envelope(),
        )
    with pytest.raises(oxedi.WriteError, match="tables must be the tables of a parse or a mapping"):
        oxedi.write([tables], envelope())
    with pytest.raises(oxedi.WriteError, match="both"):
        oxedi.write(
            tables, envelope(delimiters=oxedi.Delimiters(component=b"*", repetition=b"^"))
        )
    with pytest.raises(oxedi.WriteError, match='"delimiters.repetition" is not set'):
        oxedi.write(tables, envelope(delimiters=oxedi.Delimiters()))


def test_the_envelope_returns_its_date_time_and_options():
    given = envelope(application_sender="APP", line_break=True)
    assert given.date == datetime.date(2024, 1, 1)
    assert given.time == datetime.time(12, 30)
    assert (given.application_sender, given.application_receiver) == ("APP", None)
    assert given.line_break is True
    with pytest.raises(ValueError, match="is a datetime; give a datetime.date"):
        envelope(date=datetime.datetime(2024, 1, 1, 12, 30))
    with pytest.raises(ValueError, match="has a time zone"):
        envelope(time=datetime.time(12, 30, tzinfo=datetime.timezone.utc))


def test_the_interchange_header_drops_the_century_and_the_seconds():
    tables = parse_named("emedny_sample.txt").tables
    data = oxedi.write(
        tables, envelope(date=datetime.date(2050, 6, 15), time=datetime.time(8, 5, 9))
    )
    assert b"*500615*0805*^*" in data
    assert b"~GS*HP*SENDER*RECEIVER*20500615*080509*1*X*" in data


def test_frames_with_categories_and_timestamps_write_like_the_tables():
    import polars

    result = parse_named("emedny_sample.txt")
    tables = result.tables
    frames = {name: tables[name].to_polars() for name in tables.keys()}
    claims = frames["claims"]
    frames["claims"] = claims.with_columns(
        polars.col("claim_status").cast(polars.String).cast(polars.Categorical),
        polars.col("statement_from").cast(polars.Datetime("ns")),
    )
    assert oxedi.write(frames, envelope()) == oxedi.write(tables, envelope())
    pandas_claims = tables["claims"].to_pandas()
    pandas_claims["statement_from"] = pandas_claims["statement_from"].astype("datetime64[ns]")
    pandas_frames = {name: tables[name].to_pandas() for name in tables.keys()}
    pandas_frames["claims"] = pandas_claims
    assert oxedi.write(pandas_frames, envelope()) == oxedi.write(tables, envelope())


def test_a_float_for_a_decimal_column_is_refused():
    with pytest.raises(oxedi.WriteError, match=r"float may not hold the amount exactly; give decimal values \(decimal.Decimal"):
        oxedi.write({"claims": pyarrow.table({"charge_amount": [1.5]})}, envelope())
    with pytest.raises(oxedi.WriteError, match="is not at midnight"):
        oxedi.write(
            {"claims": pyarrow.table({"statement_from": [datetime.datetime(2024, 1, 1, 1)]})},
            envelope(),
        )


def test_a_reference_against_the_parent_chain_is_a_finding():
    result = parse_named("emedny_sample.txt")
    frames = {name: result.tables[name].to_polars() for name in result.tables.keys()}
    import polars

    services = frames["services"]
    frames["services"] = services.with_columns(
        polars.when(polars.int_range(polars.len()) == 0)
        .then(5)
        .otherwise(polars.col("payment"))
        .alias("payment")
    )
    with pytest.raises(oxedi.WriteError) as info:
        oxedi.write(frames, envelope())
    [finding] = info.value.findings
    assert finding.kind == "MismatchedReference"
    assert (finding.table, finding.row, finding.column) == ("services", 0, "payment")
