"""What the sandbox protects in a checkout: the paths bound read-only over it and the ones masked.

Read-only, in both phases: what the host executes or trusts after a run (the fixed list below),
plus every tracked file git names like a build or tool configuration and every tracked `.cargo`
directory, wherever they are. Hidden, in both phases: the maintainers' notes and every path git
ignores, except the sandbox's own outputs, so a run can neither read nor change them. An ignored
directory is masked by an empty read-only tmpfs; an ignored file has an empty read-only file
bound over it. A directory git ignores as a whole (it holds no tracked file) is masked whole,
caches inside it included.
"""

import hashlib
import os
import subprocess
from dataclasses import dataclass
from fnmatch import fnmatch
from pathlib import Path, PurePosixPath

# What the host executes or trusts after a run: bound read-only over the checkout.
HOST_EXECUTED = (
    "Makefile", "make", "tools", ".cargo", ".github", "rust-toolchain.toml", "Cargo.toml",
    "Cargo.lock", "CLAUDE.md", ".claude", "crates/oxedi_py/pyproject.toml", ".git",
)
# Tracked files with these names, anywhere, steer cargo, rustup or uv: bound read-only too.
TRACKED_NAMES = frozenset({
    "Cargo.toml", "Cargo.lock", "rust-toolchain", "rust-toolchain.toml", "pyproject.toml", "uv.toml",
    ".python-version",
})
# The maintainers' local notes: masked by an empty read-only tmpfs.
MASKED = (".doc", ".workflow-kit", ".doc/scratch")
# What a run writes on purpose: its own storage at the top, and the Python caches and the built
# extension anywhere. These ignored paths are not masked.
OUTPUT_TOP = frozenset({"target", ".venv"})
OUTPUT_NAMES = frozenset({"__pycache__", ".pytest_cache", ".mypy_cache"})
OUTPUT_FILES = "crates/oxedi_py/python/oxedi/_core*.so"
# Characters that would break a `--mount` field list or a log line.
UNSAFE_IN_PATH = (",", '"', "\n")


class SandboxError(Exception):
    """A reason the sandbox refuses to start; printed as one line, exit status 2."""


@dataclass(frozen=True)
class Checkout:
    """A checkout, the paths bound read-only over it, the directories masked and the files hidden
    (relative, `/`-separated)."""

    root: Path
    read_only: tuple[str, ...]
    masked: tuple[str, ...]
    hidden: tuple[str, ...]

    @property
    def name(self) -> str:
        return f"oxedi-sandbox-{path_hash(self.root)}"


def path_hash(path: Path) -> str:
    return hashlib.sha256(str(path).encode()).hexdigest()[:12]


def refuse_unsafe(path: Path | str, what: str) -> None:
    if any(c in str(path) for c in UNSAFE_IN_PATH):
        raise SandboxError(f"refused: the {what} path {str(path)!r} holds a comma, a quote or a line break")


def inside(path: str, outers: set[str]) -> bool:
    return any(path.startswith(f"{outer}/") for outer in outers)


def covered(path: str, outers: set[str]) -> bool:
    return path in outers or inside(path, outers)


def outermost(paths: set[str]) -> set[str]:
    """The paths that are not inside another one of them: an outer mount already covers them."""
    return {p for p in paths if not inside(p, paths)}


def git_paths(root: Path, *args: str) -> set[str]:
    result = subprocess.run(["git", "-C", str(root), *args, "-z"], capture_output=True, check=False)
    if result.returncode != 0:
        stderr = os.fsdecode(result.stderr).strip()
        raise SandboxError(f"git {' '.join(args)} failed in {root}; the sandbox needs a git checkout: {stderr}")
    return {os.fsdecode(p).rstrip("/") for p in result.stdout.split(b"\0") if p}


def configuration(tracked: set[str]) -> set[str]:
    """Tracked files named like a build or tool configuration, and the `.cargo` directories that
    hold tracked files."""
    named = {p for p in tracked if PurePosixPath(p).name in TRACKED_NAMES}
    cargo = {p[: p.index(".cargo") + len(".cargo")] for p in tracked if ".cargo" in PurePosixPath(p).parts}
    return named | cargo


def is_output(path: str) -> bool:
    parts = PurePosixPath(path).parts
    return parts[0] in OUTPUT_TOP or bool(OUTPUT_NAMES.intersection(parts)) or fnmatch(path, OUTPUT_FILES)


def ignored_paths(root: Path) -> set[str]:
    """What git ignores in `root` before the run (a wholly ignored directory as one entry), less
    the sandbox's outputs."""
    listed = git_paths(root, "ls-files", "--others", "--ignored", "--exclude-standard", "--directory")
    return {p for p in listed if not is_output(p)}


def present_real(root: Path, paths: set[str]) -> set[str]:
    """The paths that exist; a symbolic link or an unsafe name among them is refused."""
    present = {p for p in paths if os.path.lexists(root / p)}
    for path in sorted(present):
        refuse_unsafe(path, "checkout")
        if (root / path).is_symlink():
            raise SandboxError(f"refused: {path} is a symbolic link; the read-only and masked paths must be real files")
    return present


def survey(root: Path) -> Checkout:
    """The read-only, masked and hidden paths of `root`, as git and the fixed lists name them. A
    symbolic link among them is refused: its target, not the checkout's file, would be bound, and
    the link itself would stay writable."""
    refuse_unsafe(root, "checkout")
    secret = set(MASKED) | ignored_paths(root)
    present = present_real(root, set(HOST_EXECUTED) | configuration(git_paths(root, "ls-files")) | secret)
    masked = outermost({p for p in present & secret if (root / p).is_dir()})
    # What a masked directory does not already cover: ignored files are hidden, the rest bound.
    free = {p for p in present if not covered(p, masked)}
    hidden = free & secret
    read_only = outermost(free - hidden)
    return Checkout(root, tuple(sorted(read_only)), tuple(sorted(masked)), tuple(sorted(hidden)))
