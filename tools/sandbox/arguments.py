"""The `docker run` arguments of each phase: one hardened base plus what the phase changes."""

from dataclasses import dataclass
from enum import Enum
from pathlib import Path

from layout import Checkout
from storage import Storage

WORK = "/work"
HOME = "/tmp/home"
CARGO_HOME = "/sandbox/cargo"
UV_CACHE = "/sandbox/uv"
TMP_MOUNT = "/tmp:rw,nosuid,nodev,size=2g"
VENV = f"{WORK}/.venv"
# The Python the venv is made with, and the dependency group the fetch installs into it.
VENV_PYTHON = "3.13"
DEV_GROUP = "crates/oxedi_py/pyproject.toml:dev"



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


def read_only_set(checkout: Checkout, storage: Storage, skip: tuple[str, ...]) -> list[str]:
    """The read-only binds, the masked directories and the empty file over each hidden one."""
    binds = [bind(checkout.root / p, f"{WORK}/{p}", True) for p in checkout.read_only if p not in skip]
    masks = [mask(f"{WORK}/{p}") for p in checkout.masked]
    blanks = [bind(storage.empty, f"{WORK}/{p}", True) for p in checkout.hidden]
    return [a for mount in binds + masks + blanks for a in mount]


def fetch_delta(checkout: Checkout, storage: Storage) -> list[str]:
    """Network on, the checkout read-only with .cargo masked (no repository cargo configuration
    applies), the shared caches writable, rustc wrappers cleared."""
    cargo_mask = mask(f"{WORK}/.cargo") if ".cargo" in checkout.read_only else []
    return [
        *env_args({
            "HOME": HOME, "CARGO_HOME": CARGO_HOME, "CARGO_BUILD_RUSTC_WRAPPER": "",
            "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER": "", "UV_CACHE_DIR": UV_CACHE,
            # The cache and the venv are separate mounts, so uv copies instead of hardlinking.
            "UV_LINK_MODE": "copy",
            # A uv.toml or [tool.uv] the run phase left in the checkout must not steer the fetch.
            "UV_NO_CONFIG": "1",
        }),
        *bind(checkout.root, WORK, True), *read_only_set(checkout, storage, (".cargo",)), *cargo_mask,
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
        *bind(checkout.root, WORK, False), *read_only_set(checkout, storage, ()),
        *bind(storage.target, f"{WORK}/target", False), *bind(storage.venv, f"{WORK}/.venv", False),
        *bind(storage.cargo, CARGO_HOME, True),
    ]


PHASE_DELTA = {Phase.FETCH: fetch_delta, Phase.RUN: run_delta}


def docker_args(
    phase: Phase, checkout: Checkout, uid: int, gid: int, runtime: Runtime, limits: Limits, storage: Storage
) -> list[str]:
    """The `docker run` options of a phase: the hardened base, then what the phase changes."""
    return hardened_base(checkout, uid, gid, runtime, limits) + PHASE_DELTA[phase](checkout, storage)


def fetch_commands() -> list[list[str]]:
    """The fetch phase, fixed here: it never runs make, a repository script, a build script or an
    sdist. `cargo fetch` builds nothing; `--no-build` lets uv install wheels only. The venv is made
    anew from the image's own CPython (`--managed-python`: the one under /opt/python), so the only
    interpreter the fetch executes is one no run has written."""
    venv = ["uv", "venv", "--managed-python", "--python", VENV_PYTHON, VENV]
    install = ["uv", "pip", "install", "--python", f"{VENV}/bin/python", "--no-build", "--group", DEV_GROUP]
    return [["cargo", "fetch", "--locked"], venv, install]


def run_command(command: str) -> list[str]:
    return ["bash", "-c", f'mkdir -p "$HOME" && {command}']
