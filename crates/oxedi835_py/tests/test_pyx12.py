"""``oxedi835.pyx12.validate`` against pyx12 itself (skipped without pyx12)."""

import io
import json
import subprocess
import sys

import pytest

pytest.importorskip("pyx12")

import oxedi835
from oxedi835.pyx12 import validate

from conftest import SAMPLES, path_of, read

EYEMED = "edi835_test_eyemed.RMT"
LEGACY = "united_healthcare_legacy_sample.txt"
CLI = (
    "import sys; from pyx12.scripts import x12valid; "
    "sys.argv = ['x12valid', '-J', sys.argv[1]]; x12valid.main()"
)


def facts(d):
    """Everything a diagnostic carries, to compare findings by value."""
    return (
        d.level,
        d.origin,
        d.kind,
        d.code,
        d.rule,
        d.segment,
        d.element,
        d.component,
        d.path,
        d.datum,
    )


def span(data, d):
    """The byte range of the segment ``d`` names, read from the document of ``data``."""
    return oxedi835.parse(data).document[d.segment].span


def is_failure(d):
    return d.origin == "pyx12" and d.level == 1 and d.code is None


def cli_count(path):
    """The number of errors in the JSON x12valid -J writes for ``path``."""
    subprocess.run([sys.executable, "-c", CLI, str(path)], check=False, capture_output=True)
    tree = json.loads(path.with_name(path.name + ".json").read_text())
    total = 0
    for isa in tree["interchanges"]:
        total += len(isa["errors"])
        for group in isa["groups"]:
            total += len(group["errors"])
            for transaction in group["transactions"]:
                total += len(transaction["errors"])
                for seg in transaction["segments"]:
                    total += len(seg["errors"])
                    total += sum(len(e["errors"]) for e in seg["elements"])
    return total


@pytest.mark.parametrize("name", SAMPLES)
def test_the_samples_have_as_many_findings_as_x12valid_reports(name, tmp_path):
    copy = tmp_path / name
    copy.write_bytes(read(name))
    findings = validate(read(name))
    assert len(findings) == cli_count(copy)
    assert all(isinstance(f, oxedi835.Diagnostic) for f in findings)
    assert all((f.origin, f.kind, f.path) == ("pyx12", "External", "") for f in findings)


@pytest.mark.parametrize("name", ["edi835_test_united.rmt", "edi835_test_davisvision.RMT"])
def test_united_and_davisvision_have_no_findings(name):
    assert validate(read(name)) == []


def test_a_finding_points_at_the_bytes_of_its_segment():
    data = read(EYEMED)
    document = oxedi835.parse(data).document
    findings = validate(data)
    assert [(f.level, f.code, f.segment, f.element, f.component) for f in findings] == [
        (2, "1", 333, 8, None),
        (2, "1", 333, 9, None),
    ]
    assert document[findings[0].segment].span == (7468, 7485)
    assert data[7468:7485] == document[333].raw
    assert data[7468:7485].startswith(b"NM1*")
    assert findings[0].rule == (
        'Mandatory data element "Identification Code Qualifier" (NM108) is missing'
        " (reported by pyx12, code 1)"
    )
    assert findings[0].datum == b""


def test_a_finding_with_a_datum_names_it():
    data = read(LEGACY)
    (finding,) = validate(data)
    assert (finding.level, finding.code, finding.segment, finding.element) == (2, "7", 15, 2)
    assert finding.datum == b"NP"
    start, end = span(data, finding)
    assert data[start:end].startswith(b"N4*")
    assert str(finding) == (
        "SNIP 2 · (NP) is not a valid code for Payee State Code (N402) (reported by pyx12, code 7)"
        ' · segment #15, element 2 · at the root · datum "NP"'
    )


@pytest.mark.parametrize("prefix", [b"\xef\xbb\xbf", b"\r\n\r\n", b"\xef\xbb\xbf\n"])
def test_leading_trivia_shifts_the_range_and_not_the_segment(prefix):
    data = read(LEGACY)
    (plain,) = validate(data)
    (shifted,) = validate(prefix + data)
    assert shifted.segment == plain.segment
    before = span(data, plain)
    assert span(prefix + data, shifted) == (before[0] + len(prefix), before[1] + len(prefix))
    start, end = span(prefix + data, shifted)
    assert (prefix + data)[start:end].startswith(b"N4*")


def test_a_crafted_file_gets_translated_findings():
    data = read(LEGACY)
    at = data.index(b"BPR*")
    bad = data[:at] + b"ZZZ*1~" + data[at:]
    findings = validate(bad)
    unknown = next(f for f in findings if f.rule.startswith("Segment ZZZ*1 not found"))
    assert (unknown.level, unknown.code, unknown.segment, unknown.element) == (2, "1", 3, None)
    start, end = span(bad, unknown)
    assert bad[start:end] == b"ZZZ*1~"
    code = next(f for f in findings if f.datum == b"NP")
    start, end = span(bad, code)
    assert bad[start:end].startswith(b"N4*")
    assert code.segment == 16
    count = next(f for f in findings if f.rule.startswith("SE count"))
    assert count.rule == (
        "SE count of 61 for SE02=000000064 is wrong. I count 62 (reported by pyx12, code 4)"
    )


def test_bytes_a_path_and_a_binary_file_give_the_same_findings(tmp_path):
    data = read(EYEMED)
    copy = tmp_path / "copy.rmt"
    copy.write_bytes(data)
    expected = [facts(f) for f in validate(data)]
    assert expected
    assert [facts(f) for f in validate(copy)] == expected
    assert [facts(f) for f in validate(str(copy))] == expected
    assert [facts(f) for f in validate(io.BytesIO(data))] == expected
    assert [facts(f) for f in validate(bytearray(data))] == expected


def test_nothing_is_written_next_to_the_input(tmp_path):
    copy = tmp_path / "copy.rmt"
    copy.write_bytes(read(EYEMED))
    validate(copy)
    assert [p.name for p in tmp_path.iterdir()] == ["copy.rmt"]


def test_a_file_pyx12_cannot_read_gives_one_failure():
    (failure,) = validate(read("multi_claim_sample.txt"))
    assert is_failure(failure)
    assert (failure.segment, failure.element, failure.datum) == (None, None, b"")
    assert failure.rule == (
        "could not finish validating: X12Error: ISA Interchange Control Version Number"
        " is unknown: 0401* for ISA*00*          *00*          *ZZ*RUSHMORE      *ZZ*ACME_MED"
        "       *190316*1615*U*00401*000001234*0*P*>~; it reached no segment (reported by pyx12)"
    )


def test_a_report_pyx12_shape_change_becomes_one_failure(monkeypatch):
    import oxedi835.pyx12._validate as module

    real = json.loads

    def reshaped(text):
        tree = real(text)
        tree["interchanges"] = [{"groups": []}]
        return tree

    monkeypatch.setattr(module.json, "loads", reshaped)
    (failure,) = validate(read(EYEMED))
    assert is_failure(failure)
    assert failure.segment is None
    assert failure.rule == (
        "could not finish validating: its report could not be translated"
        " (KeyError: 'errors'); it reached no segment (reported by pyx12)"
    )


def test_a_file_without_isa_raises_parse_error():
    with pytest.raises(oxedi835.ParseError):
        validate(read("blue_cross_nc_sample.txt"))


def test_an_exception_inside_pyx12_becomes_one_failure(monkeypatch):
    import pyx12.x12n_document

    real = pyx12.x12n_document.apply_segment_errors
    calls = []

    def breaks(node, seg, errh):
        calls.append(seg)
        if len(calls) == 6:
            raise RuntimeError("map exploded")
        return real(node, seg, errh)

    monkeypatch.setattr(pyx12.x12n_document, "apply_segment_errors", breaks)
    data = read(EYEMED)
    findings = validate(data)
    assert len(findings) == 1
    (failure,) = findings
    assert is_failure(failure)
    assert failure.rule == (
        "could not finish validating: RuntimeError: map exploded;"
        " it was processing segment #5; the last it completed is #4 (reported by pyx12)"
    )
    assert failure.segment == 5
    document = oxedi835.parse(data).document
    start, end = document[failure.segment].span
    assert data[start:end] == document[5].raw
    assert failure.datum == bytes(document[5].id)


def logger_state():
    import logging

    logger = logging.getLogger("pyx12")
    child = logging.getLogger("pyx12.error_handler")
    return [
        (lg.level, lg.disabled, lg.propagate, list(lg.handlers))
        for lg in (logger, child)
    ] + [logging.root.manager.disable]


def reference():
    return [
        facts(f) for f in validate(read(LEGACY) + b"")
    ] + [
        facts(f) for f in validate(read("multi_claim_sample.txt"))
    ]


@pytest.mark.parametrize("config", ["critical", "disable", "child_off"])
def test_results_do_not_depend_on_the_callers_logging_configuration(config):
    import logging

    expected = reference()
    logger = logging.getLogger("pyx12")
    child = logging.getLogger("pyx12.error_handler")
    before = logger_state()
    try:
        if config == "critical":
            logger.setLevel(logging.CRITICAL)
        elif config == "disable":
            logging.disable(logging.CRITICAL)
        else:
            child.disabled = True
        state = logger_state()
        assert reference() == expected
        assert logger_state() == state
    finally:
        logging.disable(logging.NOTSET)
        logger.setLevel(before[0][0])
        child.disabled = before[1][1]
    assert any("X12Error: ISA Interchange Control Version Number is unknown" in e[4] for e in expected)


def test_the_logger_is_restored_after_an_exception(monkeypatch):
    import pyx12.x12n_document

    before = logger_state()

    def breaks(*args):
        raise RuntimeError("boom")

    monkeypatch.setattr(pyx12.x12n_document, "apply_segment_errors", breaks)
    (failure,) = validate(read(EYEMED))
    assert "RuntimeError: boom" in failure.rule
    assert logger_state() == before


def test_overlapping_validations_do_not_mix_their_findings():
    from concurrent.futures import ThreadPoolExecutor

    expected = reference()
    with ThreadPoolExecutor(4) as pool:
        results = list(pool.map(lambda _: reference(), range(8)))
    assert all(r == expected for r in results)
    assert logger_state()[0][2] is True


def crafted():
    """The legacy fixture with a segment no loop holds, inserted before BPR."""
    data = read(LEGACY)
    at = data.index(b"BPR*")
    return data[:at] + b"ZZZ*1~" + data[at:]


def test_parse_and_validate_findings_mix_sort_by_level_and_filter_by_origin():
    data = crafted()
    findings = oxedi835.parse(data).diagnostics + validate(data)
    assert all(isinstance(d, oxedi835.Diagnostic) for d in findings)
    assert {d.origin for d in findings} == {"oxedi835", "pyx12"}
    ordered = sorted(findings, key=lambda d: d.level)
    levels = [d.level for d in ordered]
    assert {1, 2} <= set(levels)
    assert levels == sorted(levels)
    ours = [d for d in findings if d.origin == "oxedi835"]
    theirs = [d for d in findings if d.origin == "pyx12"]
    assert ours and theirs and len(ours) + len(theirs) == len(findings)
    assert all(d.code is None for d in ours)
    assert all(d.code is not None for d in theirs)
    assert any(d.kind == "UnknownSegment" and d.segment == 3 for d in ours)


def test_envelope_findings_are_level_1_and_segment_and_element_findings_level_2():
    findings = validate(crafted())
    by_rule = {d.rule.split(" (reported by")[0]: d for d in findings}
    transaction = by_rule["SE count of 61 for SE02=000000064 is wrong. I count 62"]
    assert (transaction.level, transaction.element) == (1, None)
    segment = next(d for d in findings if d.rule.startswith("Segment ZZZ*1 not found"))
    assert (segment.level, segment.element) == (2, None)
    element = next(d for d in findings if d.datum == b"NP")
    assert (element.level, element.element) == (2, 2)
    assert {d.level for d in findings} == {1, 2}


def test_a_failure_is_level_1_without_code():
    (failure,) = validate(read("multi_claim_sample.txt"))
    assert (failure.level, failure.code, failure.origin, failure.kind) == (
        1,
        None,
        "pyx12",
        "External",
    )
    assert failure.rule.startswith("could not finish validating")
