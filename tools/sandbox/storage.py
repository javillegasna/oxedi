"""The sandbox's storage on the host: shared caches and each checkout's target/ and venv."""

import os
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


def prepare(storage: Storage, checkout: Checkout) -> None:
    """Creates the storage and the mount points as the user, so docker never creates them as root."""
    for directory in (storage.cargo, storage.uv, storage.target, storage.venv):
        directory.mkdir(mode=0o700, parents=True, exist_ok=True)
    for mount_point in ("target", ".venv"):
        (checkout.root / mount_point).mkdir(exist_ok=True)


def remove(path: Path) -> None:
    """Removes a file, a link or a whole tree. A directory gets its owner's permissions back first,
    so one a run made unreadable still goes; links are removed, never followed."""
    if path.is_symlink() or not path.is_dir():
        path.unlink()
        return
    path.chmod(0o700)
    for entry in path.iterdir():
        remove(entry)
    path.rmdir()


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
