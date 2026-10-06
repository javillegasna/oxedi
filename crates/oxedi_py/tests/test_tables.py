import pytest

from conftest import parse_named

NAMES = ["adjustments", "claims", "payments", "provider_adjustments", "services"]


@pytest.fixture
def tables():
    return parse_named("emedny_sample.txt").tables


def test_keys_contains_and_columns(tables):
    assert tables.keys() == NAMES
    assert "claims" in tables
    assert "nope" not in tables
    assert (5 in tables) is False
    assert tables["claims"].columns[:5] == [
        "row",
        "segment",
        "payment",
        "charge_amount",
        "claim_id",
    ]


def test_an_unknown_table_names_the_tables(tables):
    with pytest.raises(KeyError) as info:
        tables["x"]
    assert info.value.args[0] == (
        'there is no table "x"; the tables are: '
        "adjustments, claims, payments, provider_adjustments, services"
    )


def test_render_is_the_tables_rendered_in_order(tables):
    assert tables.render() == "".join(tables[name].render() for name in tables.keys())


def test_reprs(tables):
    assert repr(tables) == (
        "Tables(adjustments: 4 rows, claims: 3 rows, payments: 1 rows, "
        "provider_adjustments: 0 rows, services: 10 rows)"
    )
    assert repr(tables["claims"]) == "Table(name='claims', rows=3, columns=24)"
