"""The typed reader of pyx12's JSON error output."""

import json

import pytest

from oxedi.pyx12._report import (
    ElementNode,
    Error,
    Group,
    Interchange,
    PyX12ShapeError,
    Report,
    SegmentNode,
    Transaction,
    parse_report,
)
from oxedi.pyx12._tree import collect


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


def segment_report(segment_errors, elements=(), line=16):
    node = SegmentNode(line, tuple(segment_errors), tuple(elements))
    return Report((Interchange(1, (), (Group(2, (), (Transaction(3, (), (node,)),)),)),))


def test_the_rollup_of_element_errors_is_not_a_finding():
    rollup = Error("8", "SEG_8_has_data_element_errors", "Segment has data element errors", None)
    element = ElementNode(2, None, (Error("7", "ELE_7_invalid_code", "(NP) is not a valid code", "NP"),))
    found = collect(segment_report([rollup], [element]))
    assert [(f.level, f.code, f.line, f.element) for f in found] == [(2, "7", 16, 2)]


def test_a_segment_error_with_code_8_that_is_not_the_rollup_is_kept():
    error = Error("8", "SEG_8_trailing_terminators", "trailing terminators", None)
    (finding,) = collect(segment_report([error]))
    assert finding.code == "8"
    assert finding.text == "trailing terminators (SEG_8_trailing_terminators)"


def folded_error():
    text = 'Too many elements in segment "Reassociation Trace Number" (TRN). Has 5, should have 4'
    return Error("8", "SEG_8_has_data_element_errors", text, "Y")


def test_a_folded_segment_error_is_kept():
    (finding,) = collect(segment_report([folded_error()], line=4))
    assert (finding.level, finding.code, finding.line, finding.element) == (2, "8", 4, None)
    assert finding.value == "Y"
    assert finding.text.endswith("(SEG_8_has_data_element_errors)")


def test_a_folded_segment_error_is_kept_next_to_element_errors():
    element = ElementNode(2, None, (Error("7", "ELE_7_invalid_code", "(NP) is not a valid code", "NP"),))
    found = collect(segment_report([folded_error()], [element], line=4))
    assert [(f.code, f.element) for f in found] == [("8", None), ("7", 2)]


def test_a_legacy_code_adds_no_name_to_the_message():
    element = ElementNode(1, None, (Error("6", "6", "bad char", "x"),))
    (finding,) = collect(segment_report([], [element]))
    assert finding.text == "bad char"


def test_an_envelope_error_names_pyx12s_code_too():
    error = Error("024", "ISA_024_invalid_content", "unterminated group", None)
    (finding,) = collect(Report((Interchange(1, (error,), ()),)))
    assert finding.text == "unterminated group (ISA_024_invalid_content)"
    assert finding.element is None
