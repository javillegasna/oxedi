"""The one module that runs pyx12 (skipped without pyx12)."""

import pytest

pytest.importorskip("pyx12")

from conftest import read as read_bytes
from oxedi.pyx12._pyx12 import Run, refusal, run
from oxedi.pyx12._report import parse_report

EYEMED = "edi835_test_eyemed.RMT"


def read(name):
    return read_bytes(name).decode("latin-1")


def test_a_sample_run_writes_a_report_the_reader_accepts():
    r = run(read(EYEMED))
    assert (r.failure, r.ok) == (None, False)
    assert r.report is not None and len(parse_report(r.report).interchanges) == 1
    assert r.track and list(r.track) == sorted(set(r.track))


def test_a_clean_sample_is_ok():
    assert run(read("edi835_test_united.rmt")).ok is True


def test_bytes_pyx12_cannot_read_leave_no_report():
    assert run("hello~") == Run(None, (), False, None)


def test_an_exception_inside_pyx12_is_returned_not_raised(monkeypatch):
    import pyx12.x12n_document

    real = pyx12.x12n_document.apply_segment_errors
    calls = []

    def breaks(node, seg, errh):
        calls.append(seg)
        if len(calls) == 6:
            raise RuntimeError("boom")
        return real(node, seg, errh)

    monkeypatch.setattr(pyx12.x12n_document, "apply_segment_errors", breaks)
    r = run(read(EYEMED))
    assert isinstance(r.failure, RuntimeError)
    assert r.report is None
    assert r.track == (1, 2, 3, 4, 5)


def test_a_refusal_quotes_the_x12_error():
    assert refusal("hello~").startswith("X12Error: ")
    assert refusal(read(EYEMED)) == "pyx12 does not read it as an X12 file"
