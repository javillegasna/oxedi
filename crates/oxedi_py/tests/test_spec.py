import inspect
import json
import types

import pytest

import oxedi
from conftest import LARGEST, read
from oxedi import Spec, SpecError, Version
from oxedi._core import EDI_835_PARSER_PATCH

PATCH = {"loops": {"ZZ": {"parent": "2100", "trigger": {"segment": "ZZ1"}}}}


def test_the_builtin_spec_loads_and_round_trips_through_json():
    spec = Spec.builtin()
    assert "2100" in spec.loops()
    assert Spec.from_json(spec.to_json()).to_json() == spec.to_json()


def test_a_bad_spec_raises_spec_error_with_the_core_message():
    with pytest.raises(SpecError) as info:
        Spec.from_json('{"name": "x", "loops": {"a": {"trigger": 1}}}')
    assert isinstance(info.value, ValueError)
    assert str(info.value) == (
        "spec: at loops.a.trigger: invalid type: integer `1`, expected a trigger object"
    )


@pytest.mark.parametrize(
    ("text", "message"),
    [
        ("[]", 'spec: at .: invalid type: sequence, expected a spec object with "name" and "loops"'),
        ('{"loops": {}}', "spec: at .: missing field `name`"),
        ('{"name": "x", "loops": []}', "spec: at loops: invalid type: sequence, expected a map"),
    ],
)
def test_a_top_level_fault_names_its_path(text, message):
    with pytest.raises(SpecError) as info:
        Spec.from_json(text)
    assert str(info.value) == message


def test_invalid_json_names_the_parser_message():
    with pytest.raises(SpecError) as info:
        Spec.from_json("{")
    assert str(info.value) == (
        "invalid spec JSON: EOF while parsing an object at line 1 column 1"
    )


@pytest.mark.parametrize("as_text", [False, True])
def test_a_patch_adds_a_loop(as_text):
    patch = json.dumps(PATCH) if as_text else PATCH
    patched = Spec.builtin().patch(patch)
    assert "ZZ" in patched.loops()
    assert "ZZ" not in Spec.builtin().loops()


def test_a_patch_that_breaks_the_spec_raises_with_the_inner_error():
    bad = {"loops": {"ZZ": {"parent": "nope", "trigger": {"segment": "ZZ1"}}}}
    with pytest.raises(SpecError) as info:
        Spec.builtin().patch(bad)
    assert str(info.value) == 'applying patch: loop "ZZ" names unknown parent "nope"'
    assert type(info.value).__module__ == "oxedi"


def test_a_patch_must_be_a_dict_or_a_string():
    with pytest.raises(TypeError) as info:
        Spec.builtin().patch(3)
    assert str(info.value) == "Spec.patch takes a dict or a JSON string, not int"


def _refused(patch):
    with pytest.raises(TypeError) as info:
        Spec.builtin().patch(patch)
    return str(info.value)


def test_a_set_in_a_patch_is_refused_naming_its_place():
    assert _refused({"loops": {1}}) == (
        "Spec.patch: the value at loops has type set, which is not a JSON value"
    )


def test_a_non_string_key_is_refused():
    assert _refused({"loops": {1: {}}}) == (
        "Spec.patch: a key at loops has type int; keys must be str"
    )


def test_nan_is_refused():
    assert _refused({"x": float("nan")}) == (
        "Spec.patch: the value at x is nan, which JSON cannot hold"
    )


def test_a_huge_int_is_refused():
    assert _refused({"x": 2**70}) == (
        "Spec.patch: the value at x is 1180591620717411303424, "
        "outside the 64-bit integers a spec reads"
    )


def test_bytes_are_refused():
    assert _refused({"x": [b"a"]}) == (
        "Spec.patch: the value at x[0] has type bytes, which is not a JSON value"
    )


def test_a_cycle_is_refused():
    cycle: dict = {}
    cycle["x"] = cycle
    assert _refused(cycle) == (
        "Spec.patch: the value at " + ".".join(["x"] * 128) + " nests deeper than 128 levels, "
        "which a cycle does"
    )


def test_a_tuple_and_a_mapping_proxy_are_json():
    patched = Spec.builtin().patch(types.MappingProxyType({"name": "t"}))
    assert repr(patched).startswith("Spec(name='t'")
    codes = Spec.builtin().patch({"tables": {"claims": {"loops": ("2100",)}}})
    assert isinstance(codes, Spec)


def test_the_spec_repr():
    assert repr(Spec.builtin()) == "Spec(name='835', loops=8, tables=5)"


COMPAT_TABLES = [
    "rows", "rows_adjustment_groups", "rows_adjustments", "rows_claim_adjustments",
    "rows_claim_amounts", "rows_claim_dates", "rows_claim_entities", "rows_claim_references",
    "rows_claims", "rows_interchanges", "rows_organizations", "rows_payments",
    "rows_provider_adjustments", "rows_references", "rows_remarks", "rows_service_amounts",
    "rows_service_dates",
]


def test_the_edi_835_parser_patch_loads_with_its_tables():
    from oxedi._core import EDI_835_PARSER_PATCH

    spec = Spec.builtin().patch(EDI_835_PARSER_PATCH)
    assert repr(spec) == "Spec(name='edi_835_parser', loops=8, tables=17)"
    result = oxedi.parse(read(LARGEST), spec=spec)
    assert result.tables.keys() == COMPAT_TABLES
    assert (len(result.tables["rows"]), len(result.tables["rows_claims"]), len(result.diagnostics)) == (6192, 1332, 0)


def test_the_builtins_by_version():
    five, four = Spec.builtin(version="5010"), Spec.builtin(version="4010")
    assert five.to_json() == Spec.builtin().to_json()
    assert json.loads(four.to_json())["version"]["values"][0] == "004010X091A1"
    assert json.loads(five.to_json())["version"]["values"] == ["005010X221A1"]


def test_a_version_names_its_release():
    assert str(Version.V5010) == "5010"
    assert Version.V4010.text == "4010"
    assert Version.V5010 != Version.V4010
    assert repr(Version.V5010) == "Version.V5010"
    assert repr(Version.V4010) == "Version.V4010"
    assert len({Version.V5010, Version.V5010, Version.V4010}) == 2
    assert Spec.DEFAULT_VERSION == Version.V5010
    assert Version.V5010 != 0
    assert Version.V5010 != 5010


def test_every_listed_version_is_a_member_of_the_class():
    listed = Version.all()
    assert listed
    assert listed[0] == Spec.DEFAULT_VERSION
    for version in listed:
        assert getattr(Version, repr(version).split(".")[1]) == version


def test_builtin_takes_a_version_or_its_text():
    assert Spec.builtin(Version.V4010).to_json() == Spec.builtin("4010").to_json()
    assert Spec.builtin(Version.V5010).to_json() == Spec.builtin().to_json()
    with pytest.raises(TypeError) as info:
        Spec.builtin(4010)  # type: ignore[arg-type]
    assert str(info.value) == (
        "Spec.builtin(version=4010): version must be an oxedi.Version or its text, not int"
    )


def test_a_version_whose_repr_fails_is_still_named_by_its_type():
    class Broken:
        def __repr__(self) -> str:
            raise RuntimeError("no repr")

    with pytest.raises(TypeError) as info:
        Spec.builtin(Broken())  # type: ignore[arg-type]
    assert str(info.value) == (
        "Spec.builtin(version=an object whose repr failed): version must be an oxedi.Version "
        "or its text, not test_a_version_whose_repr_fails_is_still_named_by_its_type.<locals>.Broken"
    )


def test_builtin_runtime_parameters_are_pinned():
    # The stub test allows the default's runtime text (`...`); the parameters stay pinned here.
    parameters = inspect.signature(Spec.builtin).parameters
    assert list(parameters) == ["version"]
    assert parameters["version"].kind is inspect.Parameter.POSITIONAL_OR_KEYWORD
    assert Spec.builtin().to_json() == Spec.builtin(Spec.DEFAULT_VERSION).to_json()


def test_builtin_without_a_version_is_the_default():
    assert Spec.builtin().to_json() == Spec.builtin("5010").to_json()


def test_an_unknown_builtin_version_names_the_versions_there_are():
    with pytest.raises(ValueError) as info:
        Spec.builtin(version="6020")
    assert str(info.value) == (
        'Spec.builtin(version="6020"): unknown version "6020"; '
        'version must be one of "5010", "4010"'
    )


def lines(diagnostics):
    return [str(d) for d in diagnostics]


@pytest.mark.parametrize(
    "name, version",
    [("edi835_test_davisvision.RMT", "4010"), (LARGEST, "5010")],
)
def test_without_a_spec_the_file_gets_the_builtin_of_its_version(name, version, tmp_path):
    data = read(name)
    chosen = Spec.builtin(version=version)
    expected = lines(oxedi.parse(data, spec=chosen).diagnostics)
    assert lines(oxedi.parse(data).diagnostics) == expected
    path = tmp_path / name
    path.write_bytes(data)
    assert lines(oxedi.parse_file(path).diagnostics) == expected
    streamed = [d for batch in oxedi.stream(data) for d in batch.diagnostics]
    assert lines(streamed) == expected


def test_a_given_spec_is_used_as_given():
    data = read("edi835_test_davisvision.RMT")
    kinds = [d.kind for d in oxedi.parse(data, spec=Spec.builtin()).diagnostics]
    assert kinds.count("CodeNotInList") == 2
    assert "CodeNotInList" not in [d.kind for d in oxedi.parse(data).diagnostics]
    streamed = [d.kind for b in oxedi.stream(data, spec=Spec.builtin()) for d in b.diagnostics]
    assert streamed.count("CodeNotInList") == 2


def _patched(patch):
    return lambda: Spec.builtin().patch(patch)


@pytest.mark.parametrize(
    ("make_spec", "table", "column", "expected"),
    [
        (Spec.builtin, "claims", "claim_id", (b"CLP", 1, None)),
        (Spec.builtin, "claims", "patient_last_name", (b"NM1", 3, None)),
        (Spec.builtin, "payments", "payer_name", (b"N1", 2, None)),
        (Spec.builtin, "services", "procedure_code", (b"SVC", 1, 2)),
        (Spec.builtin, "adjustments", "group_code", (b"CAS", 1, None)),
        (
            _patched(
                {
                    "tables": {
                        "services": {
                            "columns": {
                                "claim_id": {
                                    "loop": "2100",
                                    "occurrence": "claim_payment_information",
                                    "element": 1,
                                }
                            }
                        }
                    }
                }
            ),
            "services",
            "claim_id",
            (b"CLP", 1, None),
        ),
    ],
)
def test_column_source_names_the_segment_element_and_component(make_spec, table, column, expected):
    source = make_spec().column_source(table, column)
    assert (source.segment, source.element, source.component) == expected


@pytest.mark.parametrize(
    ("get", "text", "kind", "field"),
    [
        (
            lambda: Spec.builtin().column_source("services", "procedure_code"),
            "ElementSource(segment=b'SVC', element=1, component=2)",
            oxedi.ElementSource,
            "element",
        ),
        (
            lambda: Spec.builtin().group_sources("provider_adjustments")["reason_code"],
            "GroupSource(first=3, step=2, offset=0, component=1)",
            oxedi.GroupSource,
            "offset",
        ),
    ],
)
def test_a_source_is_a_frozen_value(get, text, kind, field):
    a, again = get(), get()
    assert repr(a) == text
    assert a == again
    assert hash(a) == hash(again)
    assert isinstance(a, kind)
    with pytest.raises(AttributeError):
        setattr(a, field, 3)


def test_a_spec_names_its_tables_and_the_segment_that_opens_each_loop():
    assert Spec.builtin().tables() == ["adjustments", "claims", "payments", "provider_adjustments", "services"]
    compat = Spec.builtin().patch(EDI_835_PARSER_PATCH)
    assert compat.tables() == COMPAT_TABLES
    spec = Spec.builtin()
    assert [spec.trigger_segment(n) for n in ("interchange", "2100", "1000A")] == [b"ISA", b"CLP", b"N1"]


def test_element_and_group_sources_list_the_columns_that_read_an_element():
    spec = Spec.builtin()
    fixed = spec.element_sources("provider_adjustments")
    assert {n: (s.segment, s.element, s.component) for n, s in fixed.items()} == {
        "fiscal_period_date": (b"PLB", 2, None),
        "provider_id": (b"PLB", 1, None),
    }
    groups = spec.group_sources("provider_adjustments")
    assert {n: (g.first, g.step, g.offset, g.component) for n, g in groups.items()} == {
        "amount": (3, 2, 1, None),
        "reason_code": (3, 2, 0, 1),
        "reference_id": (3, 2, 0, 2),
    }
    assert [p for p in range(1, 10) if groups["amount"].holds(p)] == [4, 6, 8]
    assert spec.group_sources("services") == {}
    assert list(spec.element_sources("services")) == [
        "allowed_amount",
        "charge_amount",
        "line_item_control_number",
        "original_units",
        "payment_amount",
        "procedure_code",
        "procedure_qualifier",
        "service_date",
        "units_paid",
    ]
    compat = spec.patch(EDI_835_PARSER_PATCH).element_sources("rows_payments")
    assert list(compat) == ["amount_paid", "payment_method", "routing_number", "transaction_date"]


@pytest.mark.parametrize(
    ("call", "text"),
    [
        (
            lambda: Spec.builtin().element_sources("claim"),
            'Spec.element_sources("claim"): table "claim" is not a table of the spec; its tables are '
            '"adjustments", "claims", "payments", "provider_adjustments", "services"',
        ),
        (
            lambda: Spec.builtin().group_sources("claim"),
            'Spec.group_sources("claim"): table "claim" is not a table of the spec; its tables are '
            '"adjustments", "claims", "payments", "provider_adjustments", "services"',
        ),
        (
            lambda: Spec.builtin().trigger_segment("ZZ"),
            'Spec.trigger_segment("ZZ"): loop "ZZ" is not a loop of the spec; its loops are '
            '"1000A", "1000B", "2000", "2100", "2110", "group", "interchange", "transaction"',
        ),
    ],
)
def test_a_lookup_by_name_refuses_an_unknown_name_and_lists_the_known_ones(call, text):
    with pytest.raises(KeyError) as info:
        call()
    assert info.value.args[0] == text


@pytest.mark.parametrize(
    ("make_spec", "table", "column", "error", "text"),
    [
        (
            Spec.builtin,
            "claim",
            "claim_id",
            KeyError,
            'Spec.column_source("claim", "claim_id"): table "claim" is not a table of the spec; '
            'its tables are "adjustments", "claims", "payments", '
            '"provider_adjustments", "services"',
        ),
        (
            Spec.builtin,
            "services",
            "code",
            KeyError,
            'Spec.column_source("services", "code"): table "services" has no column "code" in the spec; '
            'its columns are "allowed_amount", "charge_amount", '
            '"line_item_control_number", "original_units", "payment_amount", '
            '"procedure_code", "procedure_qualifier", "service_date", "units_paid"',
        ),
        (
            _patched({"tables": {"claims": {"columns": {"clp_segment": {"segment": "CLP", "segment_index": True}}}}}),
            "claims",
            "clp_segment",
            ValueError,
            'Spec.column_source("claims", "clp_segment"): column "clp_segment" of table "claims" '
            "holds the index of a CLP segment, not an element; ask for a column that reads an element",
        ),
        (
            Spec.builtin,
            "adjustments",
            "reason_code",
            ValueError,
            'Spec.column_source("adjustments", "reason_code"): column "reason_code" of table '
            '"adjustments" reads offset 0 of each repeated element group, so its element changes '
            "from row to row; ask for a column that reads one fixed element",
        ),
    ],
)
def test_column_source_refuses_with_the_call_what_and_what_to_do(make_spec, table, column, error, text):
    with pytest.raises(error) as info:
        make_spec().column_source(table, column)
    assert info.value.args[0] == text


def test_to_json_writes_the_model_not_the_text_as_written():
    canonical = json.loads(Spec.builtin().to_json())
    for loop in canonical["loops"].values():
        for occurrence in loop.get("occurrences", {}).values():
            occurrence.setdefault("usage", "situational")
    written = json.dumps(canonical)
    assert '"usage": "situational"' in written
    loaded = Spec.from_json(written)
    assert '"situational"' not in loaded.to_json()
    assert loaded.to_json() == Spec.builtin().to_json()
    assert '"situational"' not in Spec.builtin().to_json()


def test_a_patched_spec_writes_its_patch():
    patched = Spec.builtin().patch(PATCH)
    assert json.loads(patched.to_json())["loops"]["ZZ"] == {
        "parent": "2100",
        "trigger": {"segment": "ZZ1"},
    }
    assert '"situational"' not in patched.to_json()
    assert Spec.from_json(patched.to_json()).to_json() == patched.to_json()


def test_a_patch_of_a_patch_keeps_both():
    twice = Spec.builtin().patch(PATCH).patch({"name": "mine"})
    assert "ZZ" in twice.loops()
    assert json.loads(twice.to_json())["name"] == "mine"


def test_a_patch_of_a_parsed_spec_keeps_the_given_patch():
    given = Spec.builtin().patch(PATCH)
    result = oxedi.parse(read(LARGEST), spec=given)
    assert "ZZ" in result.spec.patch({"name": "mine"}).loops()
