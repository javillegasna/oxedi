import mmap
from array import array

import pytest

import oxedi835
from conftest import LARGEST, delimiters_for, parse_named, path_of, read

EMEDNY = "emedny_sample.txt"


def test_every_file_is_written_back_byte_for_byte(file_name):
    data = read(file_name)
    document = parse_named(file_name).document
    assert document.write() == data
    assert b"".join(document[i].raw for i in range(len(document))) == data


def test_segments_expose_index_id_elements_and_raw():
    document = parse_named(EMEDNY).document
    assert (document[0].index, document[0].id) == (0, b"ISA")
    assert document[0].elements[15] == [b"", b""]
    svc = next(
        document[i] for i in range(len(document)) if document[i].id == b"SVC"
    )
    assert isinstance(svc.elements[0], list)
    assert svc.elements[0][0] == b"HC"
    clps = [document[i] for i in range(len(document)) if document[i].id == b"CLP"]
    assert clps
    for clp in clps:
        assert all(isinstance(e, (bytes, list)) for e in clp.elements)
    assert document[-1].index == len(document) - 1


def test_an_index_past_the_end_names_the_length():
    document = parse_named(EMEDNY).document
    n = len(document)
    with pytest.raises(IndexError) as info:
        document[n]
    assert str(info.value) == (
        f"segment index {n} is out of range: the document has {n} segments"
    )


def test_the_delimiters_are_read_from_the_isa():
    delimiters = parse_named(EMEDNY).document.delimiters
    assert (delimiters.element, delimiters.component, delimiters.segment) == (
        b"*",
        b":",
        b"~",
    )
    assert delimiters.repetition == b"^"


def test_a_byte_order_mark_is_kept_in_the_first_segment_and_reported(file_name):
    data = read(file_name)
    if not data.lstrip().startswith(b"ISA"):
        pytest.skip("the file has no ISA to read the delimiters from")
    mark = b"\xef\xbb\xbf"
    plain, marked = oxedi835.parse(data), oxedi835.parse(mark + data)
    assert marked.document.write() == mark + data
    assert marked.document[0].raw == mark + plain.document[0].raw
    assert marked.document[0].id == b"ISA"
    assert len(marked.document) == len(plain.document)
    assert marked.tables.render() == plain.tables.render()
    first, *rest = marked.diagnostics
    assert (first.level, first.kind, first.segment, first.path, first.datum) == (
        1,
        "ByteOrderMark",
        0,
        "",
        mark,
    )
    assert str(first) == (
        "SNIP 1 · the input starts with a UTF-8 byte order mark, kept as leading "
        "trivia of the first segment · segment #0 · at the root · datum \"\\u{feff}\""
    )
    assert [str(d) for d in rest] == [str(d) for d in plain.diagnostics]


def test_input_without_an_isa_raises_parse_error_with_the_core_message():
    with pytest.raises(oxedi835.ParseError) as info:
        oxedi835.parse(b"ST*835*0001~")
    assert isinstance(info.value, ValueError)
    assert str(info.value) == (
        "input does not start with an ISA segment "
        "(found bytes [53 54 2a 38 33 35 2a 30])"
    )


def test_a_delimiter_must_be_one_byte():
    with pytest.raises(ValueError) as info:
        oxedi835.Delimiters(element=b"**")
    assert str(info.value) == (
        "delimiter element must be exactly one byte, got 2 bytes: b'**'"
    )


@pytest.mark.parametrize("kind", ["bytes", "bytearray", "memoryview", "mmap"])
def test_any_buffer_is_accepted(kind):
    data = read(LARGEST)
    if kind == "bytes":
        result = oxedi835.parse(data)
    elif kind == "bytearray":
        result = oxedi835.parse(bytearray(data))
    elif kind == "memoryview":
        result = oxedi835.parse(memoryview(data))
    else:
        with open(path_of(LARGEST), "rb") as handle:
            with mmap.mmap(handle.fileno(), 0, access=mmap.ACCESS_READ) as mapped:
                result = oxedi835.parse(mapped)
    assert result.document.write() == data


def test_text_is_refused_with_a_hint():
    with pytest.raises(TypeError) as info:
        oxedi835.parse("ISA*00")
    assert str(info.value) == (
        "the input must be bytes or another buffer, not str: "
        "open the file in binary mode or encode the text"
    )


EXPECTED = "a buffer of unsigned bytes, format 'B', 'b' or 'c'"


@pytest.mark.parametrize("function", ["parse", "stream"])
@pytest.mark.parametrize(
    ("value", "found"),
    [(array("i"), "format 'i'"), (5, "type int")],
)
def test_input_that_is_not_unsigned_bytes_names_function_argument_and_format(
    function, value, found
):
    with pytest.raises(TypeError) as info:
        getattr(oxedi835, function)(value)
    assert str(info.value) == (
        f"{function}: argument data must be {EXPECTED}; found {found}"
    )


def test_reprs_of_the_document_types():
    result = parse_named(EMEDNY)
    assert repr(result) == "Result(segments=69, tables=5, diagnostics=0)"
    assert repr(result.document) == "Document(segments=69, bytes=1813)"
    assert repr(result.document[0]) == "Segment(index=0, id=b'ISA')"
    assert repr(result.document.delimiters) == (
        "Delimiters(element=b'*', component=b':', segment=b'~', "
        "repetition=b'^', release=None)"
    )


def test_delimiters_repr_writes_a_newline_as_python_does():
    assert repr(oxedi835.Delimiters(element=b"\n", release=b"?")) == (
        "Delimiters(element=b'\\n', component=b':', segment=b'~', "
        "repetition=None, release=b'?')"
    )


def test_parse_file_reads_in_binary_mode(tmp_path):
    target = tmp_path / EMEDNY
    target.write_bytes(read(EMEDNY))
    assert oxedi835.parse_file(target).document.write() == read(EMEDNY)
