"""``oxedi.pyx12.validate`` against pyx12 itself (skipped without pyx12)."""

import io
import json
import subprocess
import sys

import pytest

pytest.importorskip("pyx12")

import oxedi
from oxedi.pyx12 import validate

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
    return oxedi.parse(data).document[d.segment].span


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
    assert all(isinstance(f, oxedi.Diagnostic) for f in findings)
    assert all((f.origin, f.kind) == ("pyx12", "External") for f in findings)
    assert all(f.path.startswith("interchange#1") for f in findings)


@pytest.mark.parametrize("name", ["edi835_test_united.rmt", "edi835_test_davisvision.RMT"])
def test_united_and_davisvision_have_no_findings(name):
    assert validate(read(name)) == []


def test_a_finding_points_at_the_bytes_of_its_segment():
    data = read(EYEMED)
    document = oxedi.parse(data).document
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
        " · segment #15, element 2 · at interchange#1/group#1/transaction#1/1000B#1"
        ' · datum "NP"'
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


def test_a_file_pyx12_cannot_read_gives_one_failure_at_its_isa():
    data = read("multi_claim_sample.txt")
    (failure,) = validate(data)
    assert is_failure(failure)
    isa = (
        b"ISA*00*          *00*          *ZZ*RUSHMORE      *ZZ*ACME_MED"
        b"       *190316*1615*U*00401*000001234*0*P*>~"
    )
    assert (failure.segment, failure.element, failure.datum) == (0, None, isa)
    assert data.startswith(isa)
    assert failure.rule == (
        "could not finish validating: X12Error: ISA Interchange Control Version Number"
        " is unknown: 0401* for ISA*00*          *00*          *ZZ*RUSHMORE      *ZZ*ACME_MED"
        "       *190316*1615*U*00401*000001234*0*P*>~; it rejected segment #0 (reported by pyx12)"
    )


def test_a_rejected_file_with_leading_trivia_keeps_the_isa_as_datum():
    data = b"\xef\xbb\xbf\r\n" + read("multi_claim_sample.txt")
    (failure,) = validate(data)
    assert failure.segment == 0
    assert failure.datum.startswith(b"ISA*00*") and failure.datum.endswith(b"*P*>~")


def test_an_error_tree_pyx12_shape_change_becomes_one_failure(monkeypatch):
    import oxedi.pyx12._validate as module

    def reshaped(handler):
        raise KeyError("errors")

    monkeypatch.setattr(module, "collect", reshaped)
    data = read(EYEMED)
    (failure,) = validate(data)
    assert is_failure(failure)
    last = len(oxedi.parse(data).document) - 1
    assert failure.segment == last
    assert failure.rule == (
        "could not finish validating: its error tree could not be read"
        f" (KeyError: 'errors'); it had read every segment; the last it completed is #{last}"
        " (reported by pyx12)"
    )
    assert failure.datum == b"IEA"


def test_a_failure_after_the_last_segment_names_the_last_one_completed(monkeypatch):
    import pyx12.x12file

    def breaks(self):
        raise RuntimeError("cleanup exploded")

    monkeypatch.setattr(pyx12.x12file.X12Reader, "cleanup", breaks)
    data = read(EYEMED)
    (failure,) = validate(data)
    last = len(oxedi.parse(data).document) - 1
    assert (failure.segment, failure.datum) == (last, b"IEA")
    assert failure.rule == (
        "could not finish validating: RuntimeError: cleanup exploded;"
        f" it had read every segment; the last it completed is #{last} (reported by pyx12)"
    )


def test_a_file_without_isa_raises_parse_error():
    with pytest.raises(oxedi.ParseError):
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
    document = oxedi.parse(data).document
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


def test_logging_disabled_by_the_caller_stays_disabled_during_the_call(monkeypatch):
    import logging

    import pyx12.x12n_document

    real = pyx12.x12n_document.apply_segment_errors
    seen = []

    def watches(node, seg, errh):
        seen.append(logging.root.manager.disable)
        return real(node, seg, errh)

    monkeypatch.setattr(pyx12.x12n_document, "apply_segment_errors", watches)
    logging.disable(logging.CRITICAL)
    try:
        assert len(validate(read(LEGACY))) == 1
    finally:
        logging.disable(logging.NOTSET)
    assert seen and set(seen) == {logging.CRITICAL}


def test_a_handler_on_a_pyx12_logger_receives_its_records():
    import logging

    class Keep(logging.Handler):
        def __init__(self):
            super().__init__(logging.ERROR)
            self.messages = []

        def emit(self, record):
            self.messages.append(record.getMessage())

    keep = Keep()
    logger = logging.getLogger("pyx12.error_handler")
    logger.addHandler(keep)
    try:
        (finding,) = validate(read(LEGACY))
    finally:
        logger.removeHandler(keep)
    assert finding.code == "7"
    assert keep.messages == [
        "Line:16 ELE:7 - (NP) is not a valid code for Payee State Code (N402) (NP)"
    ]


def test_the_logger_is_left_alone_after_an_exception(monkeypatch):
    import pyx12.x12n_document

    validate(read(LEGACY))
    before = logger_state()
    real = pyx12.x12n_document.apply_segment_errors

    def breaks(*args):
        raise RuntimeError("boom")

    monkeypatch.setattr(pyx12.x12n_document, "apply_segment_errors", breaks)
    (failure,) = validate(read(EYEMED))
    assert "RuntimeError: boom" in failure.rule
    assert logger_state() == before
    monkeypatch.setattr(pyx12.x12n_document, "apply_segment_errors", real)
    seen = handlers_seen(monkeypatch)
    plain_run(read(LEGACY))
    assert seen["main"][0]._oxedi_recording is False


def test_the_pyx12_logger_gets_a_null_handler_once():
    import logging

    validate(read(LEGACY))
    validate(read(LEGACY))
    handlers = logging.getLogger("pyx12").handlers
    assert [type(h) for h in handlers].count(logging.NullHandler) == 1


def test_validate_prints_nothing_when_no_logging_is_configured():
    script = (
        "import sys; from oxedi.pyx12 import validate; "
        "print(len(validate(open(sys.argv[1], 'rb').read())), len(validate(open(sys.argv[2], 'rb').read())))"
    )
    done = subprocess.run(
        [sys.executable, "-c", script, str(path_of(LEGACY)), str(path_of("multi_claim_sample.txt"))],
        check=True,
        capture_output=True,
        text=True,
    )
    assert (done.stdout, done.stderr) == ("1 1\n", "")


def test_records_still_reach_a_handler_configured_on_the_root():
    script = (
        "import logging, sys; logging.basicConfig(level=logging.ERROR, format='%(message)s'); "
        "from oxedi.pyx12 import validate; validate(open(sys.argv[1], 'rb').read())"
    )
    done = subprocess.run(
        [sys.executable, "-c", script, str(path_of(LEGACY))],
        check=True,
        capture_output=True,
        text=True,
    )
    assert done.stderr == (
        "Line:16 ELE:7 - (NP) is not a valid code for Payee State Code (N402) (NP)\n"
    )


def test_overlapping_validations_do_not_mix_their_findings():
    from concurrent.futures import ThreadPoolExecutor

    expected = reference()
    with ThreadPoolExecutor(4) as pool:
        results = list(pool.map(lambda _: reference(), range(8)))
    assert all(r == expected for r in results)


def plain_run(data):
    """pyx12 run directly, as user code would."""
    import pyx12.params
    import pyx12.x12n_document

    pyx12.x12n_document.x12n_document(
        param=pyx12.params.ParamsBase(),
        src_file=io.StringIO(data.decode("latin-1")),
        fd_997=None,
        fd_html=None,
    )


def handlers_seen(monkeypatch, on_main=None):
    """The error handlers pyx12 passes to its segment checks, by thread;
    ``on_main`` runs once on the main thread at the first check there."""
    import threading

    import pyx12.x12n_document

    real = pyx12.x12n_document.apply_segment_errors
    main = threading.current_thread()
    seen = {"main": [], "other": []}

    def watches(node, seg, errh):
        here = "main" if threading.current_thread() is main else "other"
        if here == "main" and on_main is not None and not seen["main"]:
            on_main()
        seen[here].append(errh)
        return real(node, seg, errh)

    monkeypatch.setattr(pyx12.x12n_document, "apply_segment_errors", watches)
    return seen


def test_a_plain_pyx12_run_is_not_recorded(monkeypatch):
    import pyx12.error_handler

    validate(read(LEGACY))
    seen = handlers_seen(monkeypatch)
    plain_run(read(LEGACY))
    handler = seen["main"][0]
    assert isinstance(handler, pyx12.error_handler.err_handler)
    assert handler._oxedi_recording is False
    assert handler.oxedi_dropped == [] and handler.oxedi_element_lines == {}
    assert handler.get_error_count() == 1


def test_a_pyx12_run_on_another_thread_during_validate_is_not_recorded(monkeypatch):
    import threading

    def another():
        thread = threading.Thread(target=plain_run, args=(read(EYEMED),))
        thread.start()
        thread.join()

    seen = handlers_seen(monkeypatch, on_main=another)
    (finding,) = validate(read(LEGACY))
    assert finding.code == "7"
    assert seen["main"][0]._oxedi_recording is True
    assert seen["other"] and seen["other"][0]._oxedi_recording is False
    assert seen["other"][0].get_error_count() == 1


def test_a_reloaded_error_handler_module_is_patched_again():
    import importlib

    import pyx12.error_handler

    validate(read(LEGACY))
    importlib.reload(pyx12.error_handler)
    assert not getattr(pyx12.error_handler.err_handler, "_oxedi_recorder", False)
    assert [facts(f) for f in validate(read(LEGACY))] == [facts(f) for f in validate(read(LEGACY))]
    assert getattr(pyx12.error_handler.err_handler, "_oxedi_recorder", False)
    (finding,) = validate(read(LEGACY))
    assert finding.code == "7"


def test_the_patch_is_installed_once_and_keeps_the_original_name():
    import pyx12.error_handler

    from oxedi.pyx12 import _capture

    validate(read(LEGACY))
    patched = pyx12.error_handler.err_handler
    _capture.install(pyx12.error_handler)
    assert pyx12.error_handler.err_handler is patched
    assert patched.__name__ == "err_handler"
    assert patched.__module__ == "pyx12.error_handler"


def balanced_5010():
    return read("balanced_5010_sample.txt")


def test_an_element_error_of_the_transaction_header_is_reported():
    data = balanced_5010()
    (finding,) = validate(data)
    assert (finding.level, finding.code, finding.segment, finding.element, finding.component) == (
        2,
        "10",
        2,
        3,
        None,
    )
    assert oxedi.parse(data).document[2].id == b"ST"
    assert finding.datum == b"005010X221A1"
    assert finding.rule == (
        'Data element "Implementation Convention Reference" (ST03) is marked as Not Used'
        " (reported by pyx12, code 10)"
    )


def test_an_element_error_of_the_transaction_trailer_is_reported_at_the_trailer():
    data = balanced_5010().replace(b"SE*58*0001", b"SE*5X*0001")
    findings = validate(data)
    document = oxedi.parse(data).document
    trailer = next(f for f in findings if f.code == "6")
    assert document[trailer.segment].id == b"SE"
    assert (trailer.level, trailer.element, trailer.datum) == (2, 1, b"5X")
    assert trailer.rule.startswith('Data element "Transaction Segment Count" (SE01) is type N0')


def test_segment_errors_pyx12_cannot_attach_to_an_envelope_node_are_reported():
    data = balanced_5010().replace(
        b"ST*835*0001*005010X221A1~", b"ST*835*0001*005010X221A1*~"
    )
    findings = validate(data)
    on_header = [(f.level, f.code, f.segment, f.element) for f in findings if f.element is None]
    assert on_header == [(2, "SEG1", 2, None), (2, "8", 2, None)]
    assert [f.datum for f in findings if f.element is None] == [b"ST", b"ST"]
    assert findings[1].rule == (
        "Segment contains trailing element terminators (reported by pyx12, code SEG1)"
    )
    assert findings[2].rule == (
        'Too many elements in segment "Transaction Set Header" (ST). Has 4, should have 3'
        " (reported by pyx12, code 8)"
    )


def test_findings_come_in_segment_order():
    data = balanced_5010().replace(b"SE*58*0001", b"SE*5X*0001*X")
    segments = [f.segment for f in validate(data)]
    assert segments == sorted(segments)


def crafted():
    """The legacy fixture with a segment no loop holds, inserted before BPR."""
    data = read(LEGACY)
    at = data.index(b"BPR*")
    return data[:at] + b"ZZZ*1~" + data[at:]


def test_parse_and_validate_findings_mix_sort_by_level_and_filter_by_origin():
    data = crafted()
    findings = oxedi.parse(data).diagnostics + validate(data)
    assert all(isinstance(d, oxedi.Diagnostic) for d in findings)
    assert {d.origin for d in findings} == {"oxedi", "pyx12"}
    ordered = sorted(findings, key=lambda d: d.level)
    levels = [d.level for d in ordered]
    assert {1, 2} <= set(levels)
    assert levels == sorted(levels)
    ours = [d for d in findings if d.origin == "oxedi"]
    theirs = [d for d in findings if d.origin == "pyx12"]
    assert ours and theirs and len(ours) + len(theirs) == len(findings)
    assert all(d.code is None for d in ours)
    assert all(d.code is not None for d in theirs)
    assert any(d.kind == "UnknownSegment" and d.segment == 3 for d in ours)


def test_envelope_findings_are_level_1_and_segment_and_element_findings_level_2():
    findings = validate(crafted())
    by_rule = {d.rule.split(" (reported by")[0]: d for d in findings}
    transaction = by_rule["SE count of 61 for SE02=000000064 is wrong. I count 62"]
    assert (transaction.level, transaction.element, transaction.datum) == (1, 1, b"61")
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


def test_a_text_mode_file_names_binary_mode():
    text = io.StringIO(read(EYEMED).decode("latin-1"))
    with pytest.raises(TypeError, match="opened in binary mode; the file returned str"):
        validate(text)


def renumbered(data, number):
    """``data`` with its interchange control number (ISA13 and IEA02) replaced."""
    elements = data.split(b"*")
    old = elements[13]
    return data.replace(b"*" + old + b"*", b"*" + number + b"*").replace(
        b"IEA*1*" + old, b"IEA*1*" + number
    )


def test_a_finding_in_a_second_interchange_maps_to_its_own_segment():
    first = read("edi835_test_davisvision.RMT")
    second = renumbered(first, b"%09d" % 987654321)
    assert second != first
    bad = second[: second.index(b"BPR*")] + b"ZZZ*1~" + second[second.index(b"BPR*") :]
    data = first + bad
    document = oxedi.parse(data).document
    assert sum(1 for i in range(len(document)) if document[i].id == b"ISA") == 2
    assert validate(first + second) == []
    findings = validate(data)
    unknown = next(f for f in findings if f.rule.startswith("Segment ZZZ*1 not found"))
    planted = len(first) + bad.index(b"ZZZ*1~")
    assert unknown.segment is not None
    # a segment's raw bytes hold the line break that precedes it
    start, end = document[unknown.segment].span
    assert end == planted + len(b"ZZZ*1~")
    assert start <= planted
    assert data[start:end].lstrip(b"\r\n") == b"ZZZ*1~"
    assert document[unknown.segment].raw == data[start:end]
    assert unknown.segment > len(oxedi.parse(first).document)


def reports_error(node):
    """True when any nested ``errors`` list in pyx12's JSON report is non-empty."""
    if isinstance(node, dict):
        return bool(node.get("errors")) or any(reports_error(v) for v in node.values())
    if isinstance(node, list):
        return any(reports_error(v) for v in node)
    return False


def test_user_configuration_does_not_change_the_findings(tmp_path, monkeypatch):
    import pyx12.params

    # Lowercase text is the probe: basic charset B rejects it, the default
    # extended charset E accepts it. validate() must report what the defaults
    # report (nothing here) even though pyx12, given this user's configuration,
    # would reject the same bytes.
    data = read("edi835_test_davisvision.RMT").replace(b"SILVER OAK", b"silver oak")
    monkeypatch.setenv("HOME", str(tmp_path / "empty"))
    expected = [facts(f) for f in validate(data)]
    assert expected == []
    home = tmp_path / "home"
    home.mkdir()
    (home / ".pyx12.conf.xml").write_text(
        '<?xml version="1.0" encoding="utf-8"?>'
        '<pyx12><param name="charset"><value>B</value><type>string</type></param></pyx12>'
    )
    monkeypatch.setenv("HOME", str(home))
    # the file is read by pyx12 itself, so the test means something
    assert pyx12.params.params().get("charset") == "B"
    # pyx12 run with that configuration does reject the probe
    import pyx12.x12n_document

    errors = io.StringIO()
    pyx12.x12n_document.x12n_document(
        param=pyx12.params.params(),
        src_file=io.StringIO(data.decode("latin-1")),
        fd_997=None,
        fd_html=None,
        fd_json=errors,
    )
    assert reports_error(json.loads(errors.getvalue()))
    assert [facts(f) for f in validate(data)] == expected


def five_cut(start, end=None):
    """The balanced 5010 fixture without the bytes from ``start`` up to ``end``
    (both searched for), or to its end."""
    data = balanced_5010()
    at = data.index(start)
    return data[:at] + (data[data.index(end):] if end is not None else b"")


def two_groups():
    """The balanced 5010 fixture with its group repeated, control number included."""
    data = balanced_5010()
    group = data[data.index(b"GS*") : data.index(b"IEA*")]
    return data.replace(group, group + group).replace(b"IEA*1*", b"IEA*2*")


def two_transactions():
    """The balanced 5010 fixture with its transaction set repeated, control number included."""
    data = balanced_5010()
    transaction = data[data.index(b"ST*") : data.index(b"GE*")]
    return data.replace(transaction, transaction + transaction).replace(b"GE*1*", b"GE*2*")


# Each envelope error code of pyx12, on a file that raises it, with the
# segment it lands on (by id and occurrence), the element and the datum.
ENVELOPE_CASES = {
    "ST 4": (lambda: balanced_5010().replace(b"SE*58*", b"SE*57*"), "4", (b"SE", 0), 1, b"57"),
    "ST 3": (lambda: balanced_5010().replace(b"SE*58*0001", b"SE*58*0009"), "3", (b"SE", 0), 2, b"0009"),
    "ST 23": (two_transactions, "23", (b"ST", 1), 2, b"0001"),
    "ST 2": (lambda: five_cut(b"SE*58"), "2", (b"ST", 0), 2, b"0001"),
    "GS 5": (lambda: balanced_5010().replace(b"GE*1*101", b"GE*2*101"), "5", (b"GE", 0), 1, b"2"),
    "GS 4": (lambda: balanced_5010().replace(b"GE*1*101", b"GE*1*102"), "4", (b"GE", 0), 2, b"102"),
    "GS 3 at the trailer": (lambda: five_cut(b"SE*58", b"GE*"), "3", (b"GE", 0), None, b"GE"),
    "GS 3 without a trailer": (lambda: five_cut(b"GE*"), "3", (b"GS", 0), None, b"GS"),
    "GS 6": (two_groups, "6", (b"GS", 1), 6, b"101"),
    "ISA 021": (lambda: balanced_5010().replace(b"IEA*1*", b"IEA*2*"), "021", (b"IEA", 0), 1, b"2"),
    "ISA 001": (
        lambda: balanced_5010().replace(b"IEA*1*000000101", b"IEA*1*000000102"),
        "001",
        (b"IEA", 0),
        2,
        b"000000102",
    ),
    "ISA 024": (lambda: five_cut(b"GE*", b"IEA*"), "024", (b"IEA", 0), None, b"IEA"),
    "ISA 023": (lambda: five_cut(b"IEA*"), "023", (b"ISA", 0), 13, b"000000101"),
    "ISA 025": (lambda: balanced_5010() + balanced_5010(), "025", (b"ISA", 1), 13, b"000000101"),
}


@pytest.mark.parametrize("case", sorted(ENVELOPE_CASES))
def test_an_envelope_finding_lands_where_its_code_places_it(case):
    make, code, (segment_id, occurrence), element, datum = ENVELOPE_CASES[case]
    data = make()
    document = oxedi.parse(data).document
    matching = [f for f in validate(data) if f.level == 1 and f.code == code]
    assert len(matching) == 1, matching
    (finding,) = matching
    with_id = [i for i in range(len(document)) if document[i].id == segment_id]
    assert finding.segment == with_id[occurrence]
    assert (finding.element, finding.datum) == (element, datum)
    if element is not None:
        assert document[finding.segment].elements[element - 1] == datum


def test_every_placed_code_is_pinned_by_a_case():
    from oxedi.pyx12._tree import PLACES

    pinned = {(case.split()[0], ENVELOPE_CASES[case][1]) for case in ENVELOPE_CASES}
    assert pinned == set(PLACES)


@pytest.mark.parametrize(
    "make",
    [crafted, lambda: read(EYEMED), lambda: balanced_5010().replace(b"SE*58*", b"SE*57*")],
    ids=["crafted", "eyemed", "se-count"],
)
def test_a_finding_names_the_loops_parse_names_at_its_segment(make):
    data = make()
    result = oxedi.parse(data)
    findings = validate(data)
    assert findings
    paths = oxedi._core._loop_paths(result, [f.segment for f in findings])
    for finding, path in zip(findings, paths):
        assert finding.path == "/".join(f"{name}#{ordinal}" for name, ordinal in path)
    ours = {d.segment: d.path for d in result.diagnostics if d.segment is not None}
    shared = [f for f in findings if f.segment in ours]
    assert all(f.path == ours[f.segment] for f in shared)


def test_the_crafted_findings_name_their_loops():
    paths = {(f.code, f.segment): f.path for f in validate(crafted())}
    assert paths == {
        ("1", 3): "interchange#1/group#1/transaction#1",
        ("7", 16): "interchange#1/group#1/transaction#1/1000B#1",
        ("4", 63): "interchange#1/group#1/transaction#1",
    }


def test_a_failure_names_the_loops_at_its_segment():
    (failure,) = validate(read("multi_claim_sample.txt"))
    assert failure.path == "interchange#1"
