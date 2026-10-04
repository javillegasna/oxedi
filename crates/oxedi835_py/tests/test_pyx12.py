"""``oxedi835.pyx12.validate`` against pyx12 itself (skipped without pyx12)."""

import io
import json
import subprocess
import sys

import pytest

pytest.importorskip("pyx12")

import oxedi835
from oxedi835.pyx12 import Pyx12Diagnostic, validate

from conftest import SAMPLES, path_of, read

EYEMED = "edi835_test_eyemed.RMT"
LEGACY = "united_healthcare_legacy_sample.txt"
CLI = (
    "import sys; from pyx12.scripts import x12valid; "
    "sys.argv = ['x12valid', '-J', sys.argv[1]]; x12valid.main()"
)


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
    assert all(isinstance(f, Pyx12Diagnostic) and f.origin == "pyx12" for f in findings)


@pytest.mark.parametrize("name", ["edi835_test_united.rmt", "edi835_test_davisvision.RMT"])
def test_united_and_davisvision_have_no_findings(name):
    assert validate(read(name)) == []


def test_a_finding_points_at_the_bytes_of_its_segment():
    data = read(EYEMED)
    document = oxedi835.parse(data).document
    findings = validate(data)
    assert [(f.kind, f.code, f.segment, f.element, f.component) for f in findings] == [
        ("Pyx12ElementError", "1", 333, 8, None),
        ("Pyx12ElementError", "1", 333, 9, None),
    ]
    assert findings[0].span == (7468, 7485)
    assert data[7468:7485] == document[333].raw
    assert data[7468:7485].startswith(b"NM1*")
    assert findings[0].segment_name == "Service Provider Name"
    assert findings[0].rule == (
        'Mandatory data element "Identification Code Qualifier" (NM108) is missing'
    )
    assert findings[0].datum == b""


def test_a_finding_with_a_datum_names_it():
    data = read(LEGACY)
    (finding,) = validate(data)
    assert (finding.kind, finding.code, finding.segment, finding.element) == (
        "Pyx12ElementError",
        "7",
        15,
        2,
    )
    assert finding.datum == b"NP"
    assert data[finding.span[0] : finding.span[1]].startswith(b"N4*")
    assert str(finding) == (
        "pyx12 · (NP) is not a valid code for Payee State Code (N402) (code 7)"
        f" · segment #15, bytes {finding.span[0]}..{finding.span[1]}, element 2 · datum \"NP\""
    )


@pytest.mark.parametrize("prefix", [b"\xef\xbb\xbf", b"\r\n\r\n", b"\xef\xbb\xbf\n"])
def test_leading_trivia_shifts_the_range_and_not_the_segment(prefix):
    data = read(LEGACY)
    (plain,) = validate(data)
    (shifted,) = validate(prefix + data)
    assert shifted.segment == plain.segment
    assert shifted.span == (plain.span[0] + len(prefix), plain.span[1] + len(prefix))
    start, end = shifted.span
    assert (prefix + data)[start:end].startswith(b"N4*")


def test_a_crafted_file_gets_translated_findings():
    data = read(LEGACY)
    at = data.index(b"BPR*")
    bad = data[:at] + b"ZZZ*1~" + data[at:]
    findings = validate(bad)
    unknown = next(f for f in findings if f.rule.startswith("Segment ZZZ*1 not found"))
    assert (unknown.kind, unknown.code, unknown.segment) == ("Pyx12SegmentError", "1", 3)
    assert bad[unknown.span[0] : unknown.span[1]] == b"ZZZ*1~"
    code = next(f for f in findings if f.datum == b"NP")
    assert bad[code.span[0] : code.span[1]].startswith(b"N4*")
    assert code.segment == 16
    count = next(f for f in findings if f.kind == "Pyx12TransactionError")
    assert count.rule == "SE count of 61 for SE02=000000064 is wrong. I count 62"


def test_bytes_a_path_and_a_binary_file_give_the_same_findings(tmp_path):
    data = read(EYEMED)
    copy = tmp_path / "copy.rmt"
    copy.write_bytes(data)
    expected = validate(data)
    assert validate(copy) == expected
    assert validate(str(copy)) == expected
    assert validate(io.BytesIO(data)) == expected
    assert validate(bytearray(data)) == expected


def test_nothing_is_written_next_to_the_input(tmp_path):
    copy = tmp_path / "copy.rmt"
    copy.write_bytes(read(EYEMED))
    validate(copy)
    assert [p.name for p in tmp_path.iterdir()] == ["copy.rmt"]


def test_a_file_pyx12_cannot_read_gives_one_failure():
    (failure,) = validate(read("multi_claim_sample.txt"))
    assert (failure.kind, failure.segment, failure.span, failure.code) == (
        "Pyx12Failure",
        None,
        None,
        None,
    )
    assert failure.rule == (
        "pyx12 could not finish validating: the input does not look like an X12 data file;"
        " it reached no segment"
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
    assert failure.kind == "Pyx12Failure"
    assert failure.rule == (
        "pyx12 could not finish validating: RuntimeError: map exploded;"
        " the last segment it reached is #4"
    )
    assert failure.segment == 4
    document = oxedi835.parse(data).document
    assert data[failure.span[0] : failure.span[1]] == document[4].raw
    assert failure.datum == bytes(document[4].id)


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
        (f.kind, f.segment, f.span, f.rule) for f in validate(read(LEGACY) + b"")
    ] + [
        (f.kind, f.segment, f.span, f.rule) for f in validate(read("multi_claim_sample.txt"))
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
    assert any("not look like an X12" in e[3] for e in expected)


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
