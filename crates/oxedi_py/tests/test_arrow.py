import gc
from decimal import Decimal

import polars as pl
import pyarrow as pa

import oxedi
from conftest import LARGEST, parse_named, read

GROUPS = {
    "tables": {
        "groups": {
            "loops": ["group"],
            "ref": "group",
            "columns": {
                "control": {"segment": "GS", "element": 6},
                "date": {"segment": "GS", "element": 4},
                "time": {"segment": "GS", "element": 5},
            },
        }
    }
}


def test_polars_reads_a_table_with_its_rows_and_types():
    claims = pl.DataFrame(parse_named(LARGEST).tables["claims"])
    assert claims.height == 1332
    assert claims.schema["row"] == pl.Int64
    assert claims.schema["claim_id"] == pl.Binary
    assert claims.schema["charge_amount"] == pl.Decimal(38, 2)
    assert claims.schema["statement_from"] == pl.Date
    first = claims.row(0, named=True)
    assert (first["row"], first["segment"], first["charge_amount"]) == (0, 19, Decimal("85.00"))
    assert first["statement_from"] is None


def test_every_column_type_reaches_polars_and_pyarrow():
    spec = oxedi.Spec.builtin().patch(GROUPS)
    groups = oxedi.parse(read(LARGEST), spec=spec).tables["groups"]
    frame = pl.DataFrame(groups)
    assert dict(frame.schema) == {
        "row": pl.Int64,
        "segment": pl.Int64,
        "control": pl.Int64,
        "date": pl.Date,
        "time": pl.Time,
    }
    assert str(frame.row(0, named=True)["time"]) == "11:10:00"
    schema = pa.table(groups).schema
    assert [str(t) for t in schema.types] == ["int64", "int64", "int64", "date32[day]", "time32[s]"]
    services = pa.table(parse_named(LARGEST).tables["services"]).schema
    assert services.field("charge_amount").type == pa.decimal128(38, 2)
    assert services.field("procedure_code").type == pa.binary()


def test_every_table_of_every_file_exports_with_its_row_count(file_name):
    tables = parse_named(file_name).tables
    for name in tables:
        assert pa.table(tables[name]).num_rows == len(tables[name])
        assert pa.record_batch(tables[name]).num_rows == len(tables[name])


def test_exports_share_the_column_buffers_instead_of_copying():
    claims = parse_named(LARGEST).tables["claims"]
    first = pa.table(claims).column("claim_id").chunks[0].buffers()[2]
    second = pa.table(claims).column("claim_id").chunks[0].buffers()[2]
    assert first.address == second.address


def test_exported_data_outlives_the_result():
    result = parse_named(LARGEST)
    frame = pl.DataFrame(result.tables["services"])
    table = pa.table(result.tables["services"])
    del result
    gc.collect()
    assert frame.height == 6192 and table.num_rows == 6192
    assert table.column("procedure_code")[0].as_py() == b"92015"


def test_an_integer_with_implied_decimals_keeps_its_scale_in_the_field_metadata():
    patch = dict(GROUPS, segments={"GS": {"elements": {"6": {"type": "N2"}}}})
    groups = oxedi.parse(read(LARGEST), spec=oxedi.Spec.builtin().patch(patch)).tables["groups"]
    field = pa.table(groups).schema.field("control")
    assert field.type == pa.int64()
    assert field.metadata == {b"scale": b"2"}
    assert "control: int64 (scale 2)" in groups.render()
