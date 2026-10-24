#!/usr/bin/env python3
"""A stand-in for the `docker` CLI in the sandbox tests.

Every call is appended to $FAKE_DIR/calls.jsonl as {"argv": [...]}; a `run` also records, for
each bind mount, whether its source existed and who owned it at that moment. Behaviour comes from
variables:

FAKE_INFO          the JSON `docker info --format '{{json .}}'` prints
FAKE_RUNNING       what `docker ps` prints (the running container names)
FAKE_NO_IMAGE      when set, `docker image inspect` fails until a `docker build` ran
FAKE_RUN_EXIT      exit status of the run-phase container (`bash -c ...`); default 0
FAKE_RUN_STDERR    what the run-phase container writes to stderr
FAKE_RUN_SLEEP     seconds the run-phase container sleeps before it exits
FAKE_FETCH_EXIT    exit status of every fetch-phase container; default 0

`docker kill` ends a sleeping run-phase container, as the real one would.
"""

import json
import os
import signal
import sys
import time
from itertools import pairwise
from pathlib import Path

STATE = Path(os.environ["FAKE_DIR"])
DEFAULT_INFO = {
    "SecurityOptions": ["name=apparmor,profile=default", "name=seccomp,profile=builtin", "name=cgroupns"],
    "Runtimes": {"io.containerd.runc.v2": {"path": "runc"}, "runc": {"path": "runc"}},
    "NCPU": 16,
    "MemTotal": 22914514944,
}
LABELS = {"oxedi.sandbox.rust": "1.99.0", "oxedi.sandbox.python": "3.11.15 3.13.13"}


def record(entry: dict) -> None:
    with open(STATE / "calls.jsonl", "a", encoding="utf-8") as log:
        log.write(json.dumps(entry) + "\n")


def bind_sources(argv: list[str]) -> dict:
    sources = {}
    for flag, spec in pairwise(argv):
        if flag != "--mount":
            continue
        fields = dict(part.partition("=")[::2] for part in spec.split(","))
        if fields.get("type") == "bind":
            path = Path(fields["src"])
            sources[fields["src"]] = {
                "exists": path.exists(),
                "uid": path.stat().st_uid if path.exists() else -1,
                "entries": sorted(os.listdir(path)) if path.is_dir() else [],
            }
    return sources


def image_inspect(argv: list[str]) -> int:
    if os.environ.get("FAKE_NO_IMAGE") and not (STATE / "built").exists():
        print(f"Error: No such image: {argv[-1]}", file=sys.stderr)
        return 1
    template = argv[argv.index("--format") + 1] if "--format" in argv else ""
    if "Labels" in template:
        print(json.dumps(LABELS))
    elif "Size" in template:
        print("1616000000")
    else:
        print("[]")
    return 0


def run(argv: list[str]) -> int:
    record({"argv": argv, "binds": bind_sources(argv)})
    if "cat" in argv:
        print("make\t4.3\ngcc\t13.2")
        return 0
    if "bash" not in argv:
        return int(os.environ.get("FAKE_FETCH_EXIT", "0"))
    (STATE / "run.pid").write_text(str(os.getpid()))
    time.sleep(float(os.environ.get("FAKE_RUN_SLEEP", "0")))
    sys.stderr.write(os.environ.get("FAKE_RUN_STDERR", ""))
    return int(os.environ.get("FAKE_RUN_EXIT", "0"))


def kill(argv: list[str]) -> int:
    record({"argv": argv})
    pid_file = STATE / "run.pid"
    if pid_file.exists():
        try:
            os.kill(int(pid_file.read_text()), signal.SIGTERM)
        except ProcessLookupError:
            pass
    return 0


def main(argv: list[str]) -> int:
    if argv[:1] == ["run"]:
        return run(argv)
    if argv[:1] == ["kill"]:
        return kill(argv)
    record({"argv": argv})
    if argv[:1] == ["info"]:
        print(os.environ.get("FAKE_INFO", json.dumps(DEFAULT_INFO)))
    elif argv[:2] == ["image", "inspect"]:
        return image_inspect(argv)
    elif argv[:1] == ["build"]:
        (STATE / "built").touch()
    elif argv[:1] == ["ps"]:
        print(os.environ.get("FAKE_RUNNING", ""))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
