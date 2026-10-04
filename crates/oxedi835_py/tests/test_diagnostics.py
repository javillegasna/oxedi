import oxedi835
from conftest import parse_named, read

PATH = "interchange#1/group#1/transaction#1/2000#1/2100#1/2110#1"


def first(name, kind):
    """The first diagnostic of ``kind`` on the named file."""
    return next(d for d in parse_named(name).diagnostics if d.kind == kind)


def test_a_diagnostic_carries_level_rule_location_and_datum():
    d = next(
        d
        for d in parse_named("multi_claim_sample.txt").diagnostics
        if d.kind == "RequiredElementMissing" and d.component == 2
    )
    assert (d.level, d.kind, d.segment, d.element, d.component) == (
        2,
        "RequiredElementMissing",
        25,
        1,
        2,
    )
    assert d.rule == "required element SVC01-2 (procedure_code) is missing or empty"
    assert d.path == PATH
    assert d.datum == b""
    assert str(d) == (
        "SNIP 2 · required element SVC01-2 (procedure_code) is missing or empty"
        f" · segment #25, element 1, component 2 · at {PATH} · datum \"\""
    )
    assert repr(d) == (
        "Diagnostic(level=2, kind='RequiredElementMissing', segment=25, "
        "element=1, component=2)"
    )


def test_an_unknown_segment_has_its_id_as_datum():
    d = first("multi_claim_sample.txt", "UnknownSegment")
    assert (d.level, d.kind, d.segment, d.element, d.datum) == (
        1,
        "UnknownSegment",
        19,
        None,
        b"N3",
    )


def test_a_finding_at_the_end_of_the_stream_has_no_segment():
    data = read("emedny_sample.txt")
    result = oxedi835.parse(data[: data.index(b"SE*")])
    ends = [d for d in result.diagnostics if d.segment is None]
    assert len(ends) == 3
    assert [d.kind for d in ends] == ["UnterminatedLoop"] * 3
    assert all(" · end of stream · " in str(d) for d in ends)
    assert ends[0].path == "interchange#1/group#1/transaction#1"
