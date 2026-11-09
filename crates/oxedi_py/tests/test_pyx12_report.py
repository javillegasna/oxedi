"""The typed reader of pyx12's JSON error output."""

import json

import pytest

from oxedi.pyx12._report import (
    ElementNode,
    Error,
    PyX12ShapeError,
    Report,
    SegmentNode,
    parse_report,
)


def report(**overrides):
    """One valid report; each test breaks it in one place."""
    error = {
        "err_cde": "ELE_7_invalid_code",
        "x12_code": "7",
        "err_str": "(NP) is not a valid code",
        "err_val": "NP",
    }
    element = {"ele_pos": 2, "subele_pos": None, "errors": [error]}
    segment = {"cur_line": 16, "errors": [], "elements": [element]}
    transaction = {"cur_line": 3, "errors": [], "segments": [segment]}
    group = {"cur_line": 2, "errors": [], "transactions": [transaction]}
    interchange = {"cur_line": 1, "errors": [], "groups": [group], **overrides}
    return {"interchanges": [interchange]}, segment, element, error


def read(data):
    return parse_report(json.dumps(data[0] if isinstance(data, tuple) else data)).interchanges[0]


def fails(data):
    with pytest.raises(PyX12ShapeError) as excinfo:
        parse_report(data if isinstance(data, str) else json.dumps(data[0]))
    return str(excinfo.value)


def error_of(data):
    return read(data).groups[0].transactions[0].segments[0].elements[0].errors[0]


def test_a_full_report_reads_into_typed_nodes():
    segment = read(report()).groups[0].transactions[0].segments[0]
    error = Error("7", "ELE_7_invalid_code", "(NP) is not a valid code", "NP")
    assert segment == SegmentNode(16, (), (ElementNode(2, None, (error,)),))


def test_a_report_without_x12_code_uses_the_pyx12_code():
    data = report()
    del data[3]["x12_code"]
    data[3]["err_cde"] = "7"
    assert error_of(data) == Error("7", "7", "(NP) is not a valid code", "NP")


def test_a_null_x12_code_uses_the_pyx12_code():
    data = report()
    data[3]["x12_code"] = None
    assert error_of(data).code == error_of(data).name == "ELE_7_invalid_code"


def test_an_envelope_error_has_no_value():
    data = report(errors=[{"err_cde": "021", "err_str": "count"}])
    assert read(data).errors == (Error("021", "021", "count", None),)


def test_a_missing_key_names_the_key_and_the_node():
    data = report()
    del data[1]["cur_line"]
    path = "interchanges[0].groups[0].transactions[0].segments[0]"
    assert fails(data) == f"pyx12 JSON: 'cur_line' is missing at {path}"


def test_a_wrong_type_names_the_type():
    data = report()
    data[2]["ele_pos"] = "2"
    assert fails(data).endswith("elements[0] is str, expected int")


def test_a_list_item_that_is_not_an_object_names_the_item():
    data = report()
    data[0]["interchanges"] = [[]]
    assert fails(data) == "pyx12 JSON: interchanges[0] is list, expected object"


def test_a_bool_is_not_an_int():
    data = report()
    data[2]["ele_pos"] = True
    assert fails(data).endswith("elements[0] is bool, expected int")


def test_a_missing_root_list_names_root():
    assert fails("{}") == "pyx12 JSON: 'interchanges' is missing at root"


def test_extra_keys_are_ignored():
    data = report(ta1_req="0")
    data[1]["seg_id"] = "N4"
    assert len(read(data).groups[0].transactions[0].segments[0].elements) == 1


def test_an_empty_report_has_no_interchanges():
    assert parse_report('{"interchanges": []}') == Report(())


def test_invalid_json_is_a_shape_error():
    assert fails("{").startswith("pyx12 JSON: not valid JSON: ")
