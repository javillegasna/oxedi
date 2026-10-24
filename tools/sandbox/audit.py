"""The audit after a run: what the run changed or left in the checkout, and the removal of the
caches and the built extension it may leave there.

Before the first container, `snapshot` records the untracked and ignored paths git lists and a
fingerprint of every tracked file. After the last one, `audit` compares, removes the outputs and
returns the findings. A changed tracked file or a new path outside those outputs makes the sandbox
exit 3, whatever the command's status: the host would otherwise run or commit what the run wrote.
So does an output the audit cannot remove.
"""

import hashlib
import os
import stat
import sys
from dataclasses import dataclass
from fnmatch import fnmatch
from pathlib import Path, PurePosixPath

from layout import OUTPUT_FILES, OUTPUT_NAMES, OUTPUT_TOP, Checkout, git_records, is_output
from storage import remove

DIRTY_STATUS = 3
# Untracked files one by one; an ignored directory as one entry, not its contents; submodules not
# entered, so git never reads a configuration the run could have written in one.
STATUS = ("status", "--porcelain=v1", "--untracked-files=all", "--ignored=matching", "--ignore-submodules=all")


@dataclass(frozen=True)
class Snapshot:
    """The checkout as git sees it: a fingerprint per tracked path, and the untracked and ignored
    paths (relative, `/`-separated)."""

    tracked: dict[str, str]
    others: frozenset[str]


@dataclass(frozen=True)
class Findings:
    """What a run changed (tracked files), left (new paths, described), what the audit removed and
    why it could not remove the rest."""

    changed: tuple[str, ...]
    new: tuple[str, ...]
    removed: tuple[str, ...]
    failures: tuple[str, ...]

    @property
    def dirty(self) -> bool:
        return bool(self.changed or self.new or self.failures)


def digest(path: Path) -> str:
    with open(path, "rb") as file:
        return hashlib.file_digest(file, "sha256").hexdigest()


def fingerprint(root: Path, path: str) -> str:
    """What a tracked path is now: a file's executable bits and SHA-256, a link's target, another
    type, or why it cannot be read. A path reached through a linked directory is not read."""
    full = root / path
    if os.path.realpath(full.parent) != str(full.parent):
        return "behind a symbolic link"
    try:
        info = os.lstat(full)
    except OSError as error:
        return f"unreadable: {error.strerror}"
    if stat.S_ISREG(info.st_mode):
        return f"file {info.st_mode & 0o111:o} {digest(full)}"
    if stat.S_ISLNK(info.st_mode):
        return f"link {os.readlink(full)}"
    return f"type {stat.S_IFMT(info.st_mode):o}"


def others(root: Path) -> frozenset[str]:
    """The untracked (`??`) and ignored (`!!`) paths of `git status`."""
    found = set()
    records = iter(git_records(root, *STATUS))
    for record in records:
        code, path = record[:2], record[3:]
        if code[0] in "RC":
            next(records, None)  # a rename or copy carries its old path as the next record
        if code in ("??", "!!"):
            found.add(path.rstrip("/"))
    return frozenset(found)


def snapshot(root: Path) -> Snapshot:
    tracked = git_records(root, "ls-files")
    return Snapshot({path: fingerprint(root, path) for path in tracked}, others(root))


def describe_new(root: Path, path: str) -> str:
    """The path, and where it points when it is a symbolic link out of the checkout."""
    full = root / path
    if full.is_symlink() and not Path(os.path.realpath(full)).is_relative_to(root):
        return f"{path}, a symbolic link to {os.readlink(full)} outside the checkout"
    return path


def is_scrubbed(path: str) -> bool:
    return PurePosixPath(path).name in OUTPUT_NAMES or fnmatch(path, OUTPUT_FILES)


def scrub(root: Path, skipped: set[str], removed: list[str]) -> None:
    """Removes every Python cache and built extension under `root`, never walking into a skipped
    path (what the run could not write, or its own storage), and appends each to `removed`. The
    first error stops it."""
    for directory, subdirs, files in os.walk(root):
        base = Path(directory).relative_to(root)
        doomed = [(base / name).as_posix() for name in subdirs + files if is_scrubbed((base / name).as_posix())]
        blocked = skipped.union(doomed)
        subdirs[:] = [name for name in subdirs if (base / name).as_posix() not in blocked]
        for path in doomed:
            remove(root / path)
            removed.append(path)


def audit(before: Snapshot, checkout: Checkout) -> Findings:
    """Compares the checkout with `before` and removes the outputs the run may have left."""
    root = checkout.root
    after = snapshot(root)
    paths = before.tracked.keys() | after.tracked.keys()
    changed = sorted(p for p in paths if before.tracked.get(p) != after.tracked.get(p))
    new = sorted(describe_new(root, p) for p in after.others - before.others if not is_output(p))
    # The run could not write what is read-only or masked; .git is read-only too.
    skipped = set(OUTPUT_TOP) | {".git"} | set(checkout.read_only) | set(checkout.masked)
    removed: list[str] = []
    failures: tuple[str, ...] = ()
    try:
        scrub(root, skipped, removed)
    except OSError as error:
        failures = (f"{error}; remove it, and any other Python cache or built extension in the checkout, by hand",)
    return Findings(tuple(changed), tuple(new), tuple(sorted(removed)), failures)


def report(findings: Findings) -> None:
    lines = [f"the run changed tracked file {path}" for path in findings.changed]
    lines += [f"the run left a new path {path}" for path in findings.new]
    lines += [f"cannot remove what the run left: {failure}" for failure in findings.failures]
    if findings.removed:
        lines.append(f"removed the caches and the built extension the run may have left: {', '.join(findings.removed)}")
    if findings.dirty:
        lines.append(
            f"exit {DIRTY_STATUS}: the run changed the checkout (tracked files changed: {len(findings.changed)}, "
            f"new paths: {len(findings.new)}, not removed: {len(findings.failures)}); review them with git status "
            "and git diff before running anything from this checkout"
        )
    for line in lines:
        print(f"sandbox: {line}", file=sys.stderr)
