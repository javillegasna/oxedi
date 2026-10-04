import json
import subprocess
import sys
import threading
import time
from pathlib import Path

import pytest

import oxedi835
from conftest import LARGEST, delimiters_for, parse_named, read


def repeated(name: str, copies: int) -> bytes:
    """The file with its transaction written `copies` times."""
    data = read(name)
    start = data.index(b"ST*")
    end = data.index(b"~", data.index(b"SE*")) + 1
    return b"".join([data[:start]] + [data[start:end]] * copies + [data[end:]])


def row_counts(tables) -> dict:
    return {name: len(tables[name]) for name in tables}


@pytest.mark.parametrize("by", ["transaction", "2100"])
def test_batches_add_up_to_one_parse(file_name, by):
    whole = parse_named(file_name)
    totals = {name: 0 for name in whole.tables}
    diagnostics = []
    for batch in oxedi835.stream(read(file_name), by=by, delimiters=delimiters_for(file_name)):
        for name, rows in row_counts(batch.tables).items():
            totals[name] += rows
        diagnostics.extend(str(d) for d in batch.diagnostics)
    assert totals == row_counts(whole.tables)
    assert diagnostics == [str(d) for d in whole.diagnostics]


def test_one_batch_per_transaction():
    batches = list(oxedi835.stream(repeated(LARGEST, 3)))
    assert [len(b.tables["payments"]) for b in batches] == [1, 1, 1, 0]
    assert [len(b.tables["claims"]) for b in batches] == [1332, 1332, 1332, 0]
    # The group still says it holds one transaction; that is found at GE, after the last one.
    assert [[d.kind for d in b.diagnostics] for b in batches] == [[], [], [], ["ControlCountMismatch"]]


def test_batches_render_as_the_whole_file_does():
    data = read("emedny_sample.txt")
    (batch,) = list(oxedi835.stream(data))
    assert batch.tables.render() == oxedi835.parse(data).tables.render()


def test_an_unknown_loop_names_the_loops_of_the_spec():
    with pytest.raises(ValueError) as raised:
        oxedi835.stream(read(LARGEST), by="claim")
    assert str(raised.value) == (
        'stream by "claim": the spec has no such loop; its loops are 1000A, 1000B, 2000, 2100, 2110, group, interchange, transaction'
    )


# Runs in a fresh interpreter and reads the peak resident set (VmHWM) of
# that process alone: ru_maxrss would also count the parent's at fork time.
MEASURE = """
import json, sys
sys.path.insert(0, {tests!r})
import oxedi835
from test_stream import repeated

def peak():
    with open("/proc/self/status") as status:
        line = next(line for line in status if line.startswith("VmHWM:"))
    return int(line.split()[1]) * 1024

data = repeated("edi835_test_united.rmt", {copies})
before = peak()
if {mode!r} == "parse":
    result = oxedi835.parse(data)
else:
    for batch in oxedi835.stream(data):
        pass
print(json.dumps({{"input": len(data), "extra": peak() - before}}))
"""


def peak_over_baseline(mode: str, copies: int) -> dict:
    code = MEASURE.format(tests=str(Path(__file__).parent), copies=copies, mode=mode)
    out = subprocess.run([sys.executable, "-c", code], check=True, capture_output=True, text=True)
    return json.loads(out.stdout)


@pytest.mark.skipif(sys.platform != "linux", reason="reads /proc/self/status")
def test_streaming_holds_one_transaction_not_the_file():
    parse = peak_over_baseline("parse", 20)
    stream = peak_over_baseline("stream", 20)
    mib = 1024 * 1024
    # Both copy the input once; beyond that copy, stream holds the rows of
    # one transaction and parse holds every row and every segment's span.
    stream_beyond = stream["extra"] - stream["input"]
    parse_beyond = parse["extra"] - parse["input"]
    assert stream_beyond < 8 * mib, (parse, stream)
    assert parse_beyond > 5 * max(stream_beyond, mib), (parse, stream)


def test_parse_lets_other_threads_run():
    data = repeated(LARGEST, 2)
    oxedi835.parse(data)
    parsing = threading.Event()
    done = threading.Event()
    duration = []

    def worker():
        parsing.set()
        try:
            start = time.perf_counter()
            oxedi835.parse(data)
            duration.append(time.perf_counter() - start)
        finally:
            done.set()

    thread = threading.Thread(target=worker)
    thread.start()
    parsing.wait()
    iterations = 0
    last = time.perf_counter()
    largest_gap = 0.0
    # A held GIL would stall this loop for the whole parse, so one huge gap exposes it.
    while not done.is_set():
        now = time.perf_counter()
        largest_gap = max(largest_gap, now - last)
        last = now
        iterations += 1
    thread.join()
    assert duration, "oxedi835.parse raised in the worker thread"
    assert iterations > 100, (iterations, duration[0])
    assert largest_gap < duration[0] / 4, (largest_gap, duration[0])


def test_a_stream_advanced_from_two_threads_names_the_rule():
    data = repeated(LARGEST, 10)
    expected = sum(1 for _ in oxedi835.stream(data, by="2100"))
    stream = oxedi835.stream(data, by="2100")
    barrier = threading.Barrier(2)
    messages = []
    unexpected = []
    batches = []

    def advance():
        barrier.wait()
        try:
            while True:
                try:
                    batches.append(next(stream))
                except StopIteration:
                    return
        except RuntimeError as err:
            messages.append(str(err))
        except BaseException as err:
            unexpected.append(err)

    threads = [threading.Thread(target=advance) for _ in range(2)]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()
    assert not unexpected, unexpected
    assert messages
    assert set(messages) == {
        "Stream.__next__: this stream is already being advanced by another thread; "
        "a stream is advanced from one thread at a time"
    }
    # A thread that stopped on the error left the rest to the other one.
    assert len(batches) == expected


def test_batches_concatenated_in_polars_equal_the_parsed_table():
    import polars as pl

    data = repeated(LARGEST, 3)
    parts = [pl.DataFrame(batch.tables["services"]) for batch in oxedi835.stream(data, by="2100")]
    assert pl.concat(parts).equals(pl.DataFrame(oxedi835.parse(data).tables["services"]))


def test_input_without_an_isa_raises_parse_error():
    with pytest.raises(oxedi835.ParseError) as info:
        oxedi835.stream(b"ST*835~")
    assert str(info.value) == (
        "input does not start with an ISA segment "
        "(found bytes [53 54 2a 38 33 35 7e])"
    )


def test_the_batch_repr():
    (batch,) = list(oxedi835.stream(read("emedny_sample.txt")))
    assert repr(batch) == (
        "Batch(tables=Tables(adjustments: 4 rows, claims: 3 rows, payments: 1 rows, "
        "provider_adjustments: 0 rows, services: 10 rows), diagnostics=0)"
    )
