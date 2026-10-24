"""The sandbox's storage on the host: shared caches and each checkout's target/, venv and the
empty file bound over every ignored file."""

import os
import shutil
import sys
from dataclasses import dataclass
from pathlib import Path

from layout import Checkout, SandboxError, path_hash, refuse_unsafe


@dataclass(frozen=True)
class Storage:
    """The host directories mounted into the container."""

    cache: Path
    cargo: Path
    uv: Path
    target: Path
    venv: Path
    # An empty read-only file, bound over every file git ignores so a run cannot read it.
    empty: Path


def cache_root(env: dict[str, str]) -> Path:
    """${XDG_CACHE_HOME:-$HOME/.cache}/oxedi-sandbox; a relative XDG_CACHE_HOME is ignored, as the
    XDG specification says."""
    xdg = env.get("XDG_CACHE_HOME", "")
    base = Path(xdg) if os.path.isabs(xdg) else Path(env.get("HOME", str(Path.home()))) / ".cache"
    return base / "oxedi-sandbox"


def storage_for(cache: Path, checkout: Checkout) -> Storage:
    refuse_unsafe(cache, "storage")
    own = cache / path_hash(checkout.root)
    return Storage(cache, cache / "cargo", cache / "uv", own / "target", own / "venv", own / "empty")


def prepare(storage: Storage, checkout: Checkout) -> None:
    """Creates the storage and the mount points as the user, so docker never creates them as root."""
    for directory in (storage.cargo, storage.uv, storage.target, storage.venv):
        directory.mkdir(mode=0o700, parents=True, exist_ok=True)
    for mount_point in ("target", ".venv"):
        (checkout.root / mount_point).mkdir(exist_ok=True)
    # Made anew each time, so it is empty whatever happened to it on the host.
    if os.path.lexists(storage.empty):
        remove(storage.empty)
    storage.empty.touch(mode=0o444)


def is_tree(path: Path) -> bool:
    """A real directory: a link to one is not a tree to descend."""
    return not path.is_symlink() and path.is_dir()


def remove(path: Path) -> None:
    """Removes a file, a link or a whole tree, with an explicit stack, so a tree of any depth
    goes. A directory gets its owner's permissions back before it is listed, so one a run made
    unreadable still goes; links are removed, never followed."""
    if not is_tree(path):
        path.unlink()
        return
    pending, emptied = [path], []
    while pending:
        directory = pending.pop()
        directory.chmod(0o700)
        emptied.append(directory)
        for entry in directory.iterdir():
            if is_tree(entry):
                pending.append(entry)
            else:
                entry.unlink()
    # Every directory was listed after its parent, so the reverse order removes children first.
    for directory in reversed(emptied):
        directory.rmdir()


def empty_dir(directory: Path) -> None:
    """Removes what a directory holds and keeps the directory (a mount point)."""
    try:
        directory.chmod(0o700)
        for entry in directory.iterdir():
            remove(entry)
    except OSError as error:
        raise SandboxError(
            f"cannot empty {directory} before the fetch: {error}; remove that directory by hand"
        ) from error


def make_writable(function, path: str, _error) -> None:
    """shutil.rmtree's error handler: gives the parent directory and the path their owner's
    permissions back (a run may have removed them), then retries."""
    for target in (os.path.dirname(path), path):
        if not os.path.islink(target):
            os.chmod(target, 0o700)
    function(path)


def clean(cache: Path, root: Path, everything: bool) -> Path:
    """Removes this checkout's storage, or with `everything` the whole sandbox storage, and
    returns what it removed. Links are removed, never followed."""
    doomed = cache if everything else cache / path_hash(root)
    if not os.path.lexists(doomed):
        return doomed
    handler = {"onexc": make_writable} if sys.version_info >= (3, 12) else {"onerror": make_writable}
    try:
        shutil.rmtree(doomed, **handler)
    except OSError as error:
        raise SandboxError(f"cannot remove {doomed}: {error}; remove it by hand") from error
    return doomed
