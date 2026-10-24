#!/usr/bin/env python3
"""Runs a command of this checkout in a throwaway, hardened container of the sandbox image.

The container runs as the user, with a read-only root, no capabilities, no new privileges, the
default seccomp profile and limits on processes, memory and CPUs. It sees the checkout at /work,
with the files the host executes later (Makefile, make/, tools/, ...) bound read-only on top and
the maintainers' notes masked, and its own storage under ${XDG_CACHE_HOME:-~/.cache}/oxedi-sandbox:
target/ and .venv/ per checkout, the cargo registry shared. No host variable and no part of the
host home other than that storage reaches it. The command runs with no network.

usage: sandbox.py --image TAG --checkout DIR
The command comes from SANDBOX_CMD. SANDBOX_CPUS (default: the daemon's CPUs minus one),
SANDBOX_MEMORY (GB; default 60 % of the daemon's memory) and SANDBOX_TIMEOUT (seconds; default
3600) set the limits.
Exit status: the command's; 124 when it ran past the timeout; 130 on Ctrl-C; 2 when the sandbox
refuses to start.
"""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import threading
import time
from dataclasses import dataclass
from enum import Enum
from pathlib import Path

WORK = "/work"
HOME = "/tmp/home"
CARGO_HOME = "/sandbox/cargo"
UV_CACHE = "/sandbox/uv"
TMP_MOUNT = "/tmp:rw,nosuid,nodev,size=2g"
# What the host executes or trusts after a run: bound read-only over the checkout.
HOST_EXECUTED = (
    "Makefile", "make", "tools", ".cargo", ".github", "rust-toolchain.toml", "Cargo.toml",
    "Cargo.lock", "CLAUDE.md", ".claude", "crates/oxedi_py/pyproject.toml", ".git",
)
# The maintainers' local notes: hidden behind an empty read-only tmpfs.
MASKED = (".doc", ".workflow-kit", ".doc/scratch")
# Characters that would break a `--mount` field list or a log line.
UNSAFE_IN_PATH = (",", '"', "\n")
DEFAULT_TIMEOUT_S = 3600
KILL_GRACE_S = 10
STDERR_TAIL_BYTES = 4096


class SandboxError(Exception):
    """A reason the sandbox refuses to start; printed as one line, exit status 2."""


class Phase(Enum):
    FETCH = "fetch"
    RUN = "run"


class Runtime(Enum):
    RUNC = "runc"
    RUNSC = "runsc"

    def args(self) -> list[str]:
        """runc is the daemon's default and needs no flag."""
        return ["--runtime", self.value] if self is Runtime.RUNSC else []


@dataclass(frozen=True)
class Limits:
    cpus: int
    memory_gb: int
    timeout_s: int


@dataclass(frozen=True)
class Checkout:
    """A checkout and which of the read-only and masked paths it holds (relative, `/`-separated)."""

    root: Path
    read_only: tuple[str, ...]
    masked: tuple[str, ...]

    @property
    def name(self) -> str:
        return f"oxedi-sandbox-{path_hash(self.root)}"


@dataclass(frozen=True)
class Storage:
    """The host directories mounted into the container."""

    cache: Path
    cargo: Path
    uv: Path
    target: Path
    venv: Path


@dataclass(frozen=True)
class DaemonInfo:
    rootless: bool
    runtimes: tuple[str, ...]
    cpus: int
    memory_bytes: int


def path_hash(path: Path) -> str:
    return hashlib.sha256(str(path).encode()).hexdigest()[:12]


def refuse_unsafe(path: Path, what: str) -> None:
    if any(c in str(path) for c in UNSAFE_IN_PATH):
        raise SandboxError(f"refused: the {what} path {str(path)!r} holds a comma, a quote or a line break")


def outermost(paths: tuple[str, ...]) -> tuple[str, ...]:
    """The paths that are not inside another one of them: a mask already hides what it holds."""
    return tuple(p for p in paths if not any(p.startswith(f"{outer}/") for outer in paths))


def present_in(root: Path, paths: tuple[str, ...]) -> tuple[str, ...]:
    return tuple(p for p in paths if os.path.lexists(root / p))


def survey(root: Path) -> Checkout:
    """Which read-only and masked paths `root` holds. A symbolic link among them is refused: its
    target, not the checkout's file, would be bound, and the link itself would stay writable."""
    refuse_unsafe(root, "checkout")
    read_only, masked = present_in(root, HOST_EXECUTED), present_in(root, MASKED)
    links = [p for p in read_only + masked if (root / p).is_symlink()]
    if links:
        raise SandboxError(f"refused: {links[0]} is a symbolic link; the read-only and masked paths must be real files")
    return Checkout(root, read_only, outermost(masked))


def cache_root(env: dict[str, str]) -> Path:
    """${XDG_CACHE_HOME:-$HOME/.cache}/oxedi-sandbox; a relative XDG_CACHE_HOME is ignored, as the
    XDG specification says."""
    xdg = env.get("XDG_CACHE_HOME", "")
    base = Path(xdg) if os.path.isabs(xdg) else Path(env.get("HOME", str(Path.home()))) / ".cache"
    return base / "oxedi-sandbox"


def storage_for(cache: Path, checkout: Checkout) -> Storage:
    refuse_unsafe(cache, "storage")
    own = cache / path_hash(checkout.root)
    return Storage(cache, cache / "cargo", cache / "uv", own / "target", own / "venv")


def bind(source: Path, destination: str, read_only: bool) -> list[str]:
    return ["--mount", f"type=bind,src={source},dst={destination}" + (",readonly" if read_only else "")]


def mask(destination: str) -> list[str]:
    return ["--mount", f"type=tmpfs,dst={destination},readonly,tmpfs-size=4096"]


def env_args(variables: dict[str, str]) -> list[str]:
    """Always NAME=value: a bare `--env NAME` would copy the host's value."""
    return [a for name, value in variables.items() for a in ("--env", f"{name}={value}")]


def hardened_base(checkout: Checkout, uid: int, gid: int, runtime: Runtime, limits: Limits) -> list[str]:
    memory = f"{limits.memory_gb}g"
    return [
        "--rm", "--init", "--pull", "never", "--name", checkout.name, "--user", f"{uid}:{gid}",
        "--read-only", "--cap-drop", "ALL", "--security-opt", "no-new-privileges",
        "--pids-limit", "4096", "--memory", memory, "--memory-swap", memory, "--cpus", str(limits.cpus),
        "--tmpfs", TMP_MOUNT, "--workdir", WORK, *runtime.args(),
    ]


def read_only_set(checkout: Checkout, skip: tuple[str, ...]) -> list[str]:
    binds = [bind(checkout.root / p, f"{WORK}/{p}", True) for p in checkout.read_only if p not in skip]
    masks = [mask(f"{WORK}/{p}") for p in checkout.masked]
    return [a for mount in binds + masks for a in mount]


def fetch_delta(checkout: Checkout, storage: Storage) -> list[str]:
    """Network on, the checkout read-only with .cargo masked (no repository cargo configuration
    applies), the shared caches writable, rustc wrappers cleared."""
    cargo_mask = mask(f"{WORK}/.cargo") if ".cargo" in checkout.read_only else []
    return [
        *env_args({
            "HOME": HOME, "CARGO_HOME": CARGO_HOME, "CARGO_BUILD_RUSTC_WRAPPER": "",
            "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER": "", "UV_CACHE_DIR": UV_CACHE,
        }),
        *bind(checkout.root, WORK, True), *read_only_set(checkout, (".cargo",)), *cargo_mask,
        *bind(storage.venv, f"{WORK}/.venv", False),
        *bind(storage.cargo, CARGO_HOME, False), *bind(storage.uv, UV_CACHE, False),
    ]


def run_delta(checkout: Checkout, storage: Storage) -> list[str]:
    """No network, the checkout writable except the read-only set, the cargo registry read-only,
    no shared uv cache."""
    return [
        "--network", "none",
        *env_args({
            "HOME": HOME, "CARGO_HOME": CARGO_HOME, "UV_CACHE_DIR": "/tmp/uv-cache",
            "CARGO_NET_OFFLINE": "true", "UV_OFFLINE": "1", "CARGO_INCREMENTAL": "0",
        }),
        *bind(checkout.root, WORK, False), *read_only_set(checkout, ()),
        *bind(storage.target, f"{WORK}/target", False), *bind(storage.venv, f"{WORK}/.venv", False),
        *bind(storage.cargo, CARGO_HOME, True),
    ]


PHASE_DELTA = {Phase.FETCH: fetch_delta, Phase.RUN: run_delta}


def docker_args(
    phase: Phase, checkout: Checkout, uid: int, gid: int, runtime: Runtime, limits: Limits, storage: Storage
) -> list[str]:
    """The `docker run` options of a phase: the hardened base, then what the phase changes."""
    return hardened_base(checkout, uid, gid, runtime, limits) + PHASE_DELTA[phase](checkout, storage)


def run_command(command: str) -> list[str]:
    return ["bash", "-c", f'mkdir -p "$HOME" && {command}']


def prepare(storage: Storage, checkout: Checkout) -> None:
    """Creates the storage and the mount points as the user, so docker never creates them as root."""
    for directory in (storage.cargo, storage.uv, storage.target, storage.venv):
        directory.mkdir(mode=0o700, parents=True, exist_ok=True)
    for mount_point in ("target", ".venv"):
        (checkout.root / mount_point).mkdir(exist_ok=True)


def docker(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["docker", *args], capture_output=True, text=True, check=False)


def daemon_info() -> DaemonInfo:
    if shutil.which("docker") is None:
        raise SandboxError("docker was not found on PATH; install Docker Engine (Linux) or Docker Desktop")
    result = docker("info", "--format", "{{json .}}")
    if result.returncode != 0:
        raise SandboxError(f"docker info failed; is the Docker daemon running? {result.stderr.strip()}")
    try:
        info = json.loads(result.stdout)
        return DaemonInfo(
            rootless=any("name=rootless" in option for option in info.get("SecurityOptions") or []),
            runtimes=tuple(info.get("Runtimes") or {}),
            cpus=int(info["NCPU"]),
            memory_bytes=int(info["MemTotal"]),
        )
    except (ValueError, KeyError, TypeError) as error:
        unreadable = f"docker info printed what the sandbox cannot read ({error!r}): {result.stdout[:200]}"
        raise SandboxError(unreadable) from error


def positive_knob(env: dict[str, str], name: str, unit: str, default: int) -> int:
    text = env.get(name, "")
    if not text:
        return default
    if not (text.isascii() and text.isdigit()) or int(text) < 1:
        raise SandboxError(f'{name} must be a positive whole number of {unit}, got "{text}"')
    return int(text)


def limits_from(env: dict[str, str], info: DaemonInfo) -> Limits:
    return Limits(
        cpus=positive_knob(env, "SANDBOX_CPUS", "CPUs", max(1, info.cpus - 1)),
        memory_gb=positive_knob(env, "SANDBOX_MEMORY", "GB", max(1, info.memory_bytes * 6 // (10 << 30))),
        timeout_s=positive_knob(env, "SANDBOX_TIMEOUT", "seconds", DEFAULT_TIMEOUT_S),
    )


def image_labels(image: str) -> dict[str, str]:
    result = docker("image", "inspect", "--format", "{{json .Config.Labels}}", image)
    if result.returncode != 0:
        raise SandboxError(f"image {image} not found; build it with make sandbox-image")
    try:
        labels = json.loads(result.stdout)
    except ValueError as error:
        unreadable = f"docker image inspect printed labels the sandbox cannot read: {result.stdout[:200]}"
        raise SandboxError(unreadable) from error
    return labels if isinstance(labels, dict) else {}


def refuse_if_running(name: str) -> None:
    result = docker("ps", "--filter", f"name=^{name}$", "--format", "{{.Names}}")
    if result.returncode != 0:
        raise SandboxError(f"docker ps failed, so whether {name} runs is unknown: {result.stderr.strip()}")
    if name in result.stdout.split():
        raise SandboxError(
            f"{name} is already running for this checkout; wait for it or stop it with docker kill {name}"
        )


def kill(name: str) -> None:
    """Kills the container; one that already ended (--rm removed it) is not an error."""
    result = docker("kill", name)
    if result.returncode != 0 and "No such container" not in result.stderr and "is not running" not in result.stderr:
        print(f"sandbox: docker kill {name} failed; stop it by hand: {result.stderr.strip()}", file=sys.stderr)


class Tee(threading.Thread):
    """Copies a container's stderr to ours as it comes and keeps its last bytes, which name the
    cause when docker itself fails."""

    def __init__(self, stream):
        super().__init__(daemon=True)
        self.stream = stream
        self.tail = b""

    def run(self) -> None:
        for chunk in iter(lambda: os.read(self.stream.fileno(), 65536), b""):
            sys.stderr.buffer.write(chunk)
            sys.stderr.buffer.flush()
            self.tail = (self.tail + chunk)[-STDERR_TAIL_BYTES:]

    def last_line(self) -> str:
        lines = self.tail.decode(errors="replace").strip().splitlines()
        return lines[-1] if lines else "docker printed no reason"


@dataclass(frozen=True)
class Outcome:
    status: int
    cause: str
    timed_out: bool


def run_container(argv: list[str], name: str, timeout_s: float) -> Outcome:
    """Runs `docker run ...` and waits at most `timeout_s`. Unless it ended by itself (timeout,
    Ctrl-C, any error), the container is killed before this returns or raises."""
    process = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stderr=subprocess.PIPE)
    tee = Tee(process.stderr)
    tee.start()
    ended = False
    try:
        status = process.wait(timeout=timeout_s)
        ended = True
    except subprocess.TimeoutExpired:
        return Outcome(124, "", True)
    finally:
        if not ended:
            kill(name)
            try:
                process.wait(timeout=KILL_GRACE_S)
            except subprocess.TimeoutExpired:
                process.kill()
        tee.join(timeout=KILL_GRACE_S)
    return Outcome(status, tee.last_line(), False)


def explain(outcome: Outcome, limits: Limits) -> str:
    if outcome.timed_out:
        return f"the command ran past SANDBOX_TIMEOUT={limits.timeout_s} s; the container was killed"
    if outcome.status == 125:
        return f"docker could not start the container: {outcome.cause}"
    if outcome.status == 137:
        return (f"the container was killed (exit 137), most likely by its {limits.memory_gb} GB memory limit; "
                "raise it with SANDBOX_MEMORY=<GB>")
    return f"command exited {outcome.status}"


@dataclass(frozen=True)
class Plan:
    """Everything a phase's `docker run` is built from."""

    image: str
    checkout: Checkout
    storage: Storage
    runtime: Runtime
    limits: Limits

    def argv(self, phase: Phase, command: list[str]) -> list[str]:
        options = docker_args(phase, self.checkout, os.getuid(), os.getgid(), self.runtime, self.limits, self.storage)
        return ["docker", "run", *options, self.image, *command]


def describe(plan: Plan, labels: dict[str, str]) -> str:
    rust = labels.get("oxedi.sandbox.rust", "unlabelled")
    limits = plan.limits
    return " · ".join([
        f"sandbox: runtime {plan.runtime.value}",
        f"image {plan.image} (Rust {rust})",
        f"{limits.cpus} CPUs, {limits.memory_gb} GB, {limits.timeout_s} s",
        f"storage {plan.storage.target.parent} (target, venv), shared {plan.storage.cache} (cargo, uv)",
        f"read-only: {', '.join(plan.checkout.read_only) or 'none'}",
        f"masked: {', '.join(plan.checkout.masked) or 'none'}",
    ])


def run_phase(plan: Plan, command: str) -> int:
    started = time.monotonic()
    outcome = run_container(plan.argv(Phase.RUN, run_command(command)), plan.checkout.name, plan.limits.timeout_s)
    print(f"sandbox: run took {time.monotonic() - started:.1f} s", file=sys.stderr)
    if outcome.status != 0:
        print(f"sandbox: {explain(outcome, plan.limits)}", file=sys.stderr)
    return outcome.status


def sandbox(image: str, checkout_dir: Path, env: dict[str, str]) -> int:
    command = env.get("SANDBOX_CMD", "").strip()
    if not command:
        raise SandboxError('no command: set CMD, for example make sandbox CMD="make gates"')
    info = daemon_info()
    if info.rootless:
        raise SandboxError(
            "refused: the Docker daemon is rootless (name=rootless in docker info); "
            "use a rootful Docker Engine or Docker Desktop"
        )
    runtime = Runtime.RUNSC if Runtime.RUNSC.value in info.runtimes else Runtime.RUNC
    checkout = survey(checkout_dir)
    plan = Plan(image, checkout, storage_for(cache_root(env), checkout), runtime, limits_from(env, info))
    labels = image_labels(image)
    refuse_if_running(checkout.name)
    prepare(plan.storage, checkout)
    print(describe(plan, labels), file=sys.stderr)
    return run_phase(plan, command)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(prog="sandbox.py", description=__doc__.splitlines()[0])
    parser.add_argument("--image", required=True, help="the image tag, oxedi-sandbox:<hash>")
    parser.add_argument("--checkout", required=True, type=Path, help="the checkout to mount at /work")
    args = parser.parse_args(argv)
    try:
        return sandbox(args.image, args.checkout.resolve(), dict(os.environ))
    except SandboxError as error:
        print(f"sandbox: {error}", file=sys.stderr)
        return 2
    except KeyboardInterrupt:
        print("sandbox: interrupted; the container, if it had started, was killed", file=sys.stderr)
        return 130


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
