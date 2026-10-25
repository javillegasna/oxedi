"""What a change touches, judged without running any of it: the paths whose kind lets a change run
code or steer a tool, and the packages it adds, changes or removes in Cargo.lock and in the
declared Python dependencies.

Every text is read with `git show <sha>:<path>` through `isolated_git` and parsed as data; nothing
of the clone is imported or executed. A dependency file that does not parse is a finding.
"""

import dataclasses
import os
import re
import tomllib
from collections.abc import Callable, Iterable, Iterator
from dataclasses import dataclass
from enum import Enum
from fnmatch import fnmatch
from pathlib import Path, PurePosixPath

from layout import HOST_EXECUTED, covered
from source import Change, git_bytes, git_text


class Kind(Enum):
    BUILD_SCRIPT = "a build script: cargo compiles and runs it before the crate"
    MANIFEST = "a manifest, lock or toolchain file: it chooses what is downloaded, built and run"
    HOST_EXECUTED = "a file the host executes or trusts; the sandbox keeps using the host's own copy"
    SCRIPT = "a script the build, the tests or the tools run"
    TEST_HOOK = "a test configuration that pytest loads before any test"
    PROC_MACRO = "a procedural macro crate: the compiler runs it on every crate that uses it"
    RUN_DOCS = "documentation whose code blocks make docs-test runs"
    BINARY = "a binary file, which the diff cannot show"
    SYMLINK = "a symbolic link, which can point outside the checkout"
    SUBMODULE = "a submodule, whose commit comes from another repository"

    @property
    def why(self) -> str:
        return self.value


MANIFEST_NAMES = frozenset({"Cargo.toml", "Cargo.lock", "pyproject.toml", "uv.toml", ".python-version"})
MANIFEST_PATTERNS = ("requirements*.txt", "rust-toolchain*")
SCRIPT_PATTERNS = ("*.sh", "*.py", "*.mjs", "*.js")
# The crates' own sources, the Python package's modules included: library code, not scripts.
LIBRARY_SOURCES = ("crates/*/src/*", "crates/*/python/*")
TEST_HOOK_NAMES = frozenset({"conftest.py", "pytest.ini", "setup.cfg", "tox.ini"})
SYMLINK_MODE, SUBMODULE_MODE = "120000", "160000"
# The source Cargo.lock gives a workspace member or a path crate, which it lists without one.
NO_SOURCE = "local"
REQUIREMENT = re.compile(r"\s*([A-Za-z0-9](?:[A-Za-z0-9._-]*[A-Za-z0-9])?)\s*(.*)")

# (name, old version or spec, new version or spec, source): an added package has an empty old
# side, a removed one an empty new side. The source is the registry or git URL for Cargo.lock
# (`local` for a workspace or path crate) and the declaring file for Python.
Package = tuple[str, str, str, str]


@dataclass(frozen=True)
class Touched:
    """A path the diff names, its kinds (empty when nothing about it is flagged) and git's status
    letter (A, M, D, T)."""

    path: str
    kinds: tuple[Kind, ...]
    status: str


@dataclass(frozen=True)
class PackageDiff:
    """The packages a change adds, removes and changes, each sorted, and why a dependency file
    could not be read."""

    added: tuple[Package, ...] = ()
    removed: tuple[Package, ...] = ()
    changed: tuple[Package, ...] = ()
    errors: tuple[str, ...] = ()


def total(diffs: Iterable[PackageDiff]) -> PackageDiff:
    """The diffs joined into one, each field sorted."""
    fields = [field.name for field in dataclasses.fields(PackageDiff)]
    joined: dict[str, list] = {name: [] for name in fields}
    for diff in diffs:
        for name in fields:
            joined[name].extend(getattr(diff, name))
    return PackageDiff(**{name: tuple(sorted(items)) for name, items in joined.items()})


@dataclass(frozen=True)
class Entry:
    """One diff entry: `--raw` gives the modes and the status, `--numstat` whether git sees the
    file as binary."""

    old_mode: str
    new_mode: str
    status: str
    binary: bool = False


def matches(text: str, patterns: Iterable[str]) -> bool:
    return any(fnmatch(text, pattern) for pattern in patterns)


def is_script(path: str) -> bool:
    return matches(PurePosixPath(path).name, SCRIPT_PATTERNS) and not matches(path, LIBRARY_SOURCES)


def is_run_docs(path: str) -> bool:
    return path == "README.md" or (path.startswith("docs/") and path.endswith(".md"))


def is_host_executed(path: str) -> bool:
    return covered(path, set(HOST_EXECUTED)) or ".cargo" in PurePosixPath(path).parts


# The kinds a path has by its name and place, in Kind's order. HOST_EXECUTED is left to a path
# that is neither a build script nor a manifest, which say more.
NAME_RULES: tuple[tuple[Kind, Callable[[str], bool]], ...] = (
    (Kind.BUILD_SCRIPT, lambda path: PurePosixPath(path).name == "build.rs"),
    (Kind.MANIFEST, lambda path: PurePosixPath(path).name in MANIFEST_NAMES
     or matches(PurePosixPath(path).name, MANIFEST_PATTERNS)),
    (Kind.SCRIPT, is_script),
    (Kind.TEST_HOOK, lambda path: PurePosixPath(path).name in TEST_HOOK_NAMES),
    (Kind.RUN_DOCS, is_run_docs),
)


def name_kinds(path: str) -> set[Kind]:
    kinds = {kind for kind, applies in NAME_RULES if applies(path)}
    if not kinds & {Kind.BUILD_SCRIPT, Kind.MANIFEST} and is_host_executed(path):
        kinds.add(Kind.HOST_EXECUTED)
    return kinds


def entry_kinds(entry: Entry) -> set[Kind]:
    """The kinds an entry has by its content: binary, or a symbolic link or a submodule on either
    side of the diff."""
    modes = {entry.old_mode, entry.new_mode}
    found = {Kind.SYMLINK: SYMLINK_MODE in modes, Kind.SUBMODULE: SUBMODULE_MODE in modes, Kind.BINARY: entry.binary}
    return {kind for kind, present in found.items() if present}


def is_proc_macro(manifest: bytes) -> bool:
    """A Cargo.toml whose `[lib]` sets `proc-macro = true`. One that does not parse is already a
    manifest finding, and cargo refuses it."""
    try:
        lib = tomllib.loads(manifest.decode()).get("lib", {})
    except (UnicodeDecodeError, tomllib.TOMLDecodeError):
        return False
    return isinstance(lib, dict) and any(lib.get(key) is True for key in ("proc-macro", "proc_macro"))


def entries(records: list[str]) -> dict[str, Entry]:
    """The entries of `git diff --raw --numstat -z`: per path a raw record (`:<old mode> <new mode>
    <old sha> <new sha> <status>`, then the path), then per path a numstat record
    (`<added>\\t<deleted>\\t<path>`, `-` for both when binary)."""
    found: dict[str, Entry] = {}
    remaining = iter(records)
    for record in remaining:
        if record.startswith(":"):
            old_mode, new_mode, _, _, status = record[1:].split(" ")
            found[next(remaining)] = Entry(old_mode, new_mode, status)
        else:
            added, deleted, path = record.split("\t", 2)
            found[path] = dataclasses.replace(found[path], binary=(added, deleted) == ("-", "-"))
    return found


def touched(change: Change, root: Path) -> list[Touched]:
    """Every path the change's diff (base...head) names, sorted, with its kinds."""
    output = git_bytes(root, "diff", "--no-renames", "--raw", "--numstat", "-z", "--end-of-options",
                       f"{change.base}...{change.head}")
    records = [os.fsdecode(record) for record in output.split(b"\0") if record]
    found = []
    for path, entry in sorted(entries(records).items()):
        kinds = name_kinds(path) | entry_kinds(entry)
        if PurePosixPath(path).name == "Cargo.toml" and entry.status != "D" \
                and is_proc_macro(git_bytes(root, "show", f"{change.head}:{path}")):
            kinds.add(Kind.PROC_MACRO)
        found.append(Touched(path, tuple(kind for kind in Kind if kind in kinds), entry.status))
    return found


def table(value: object, where: str) -> dict:
    if not isinstance(value, dict):
        raise ValueError(f"{where} is not a table")
    return value


def strings(value: object, where: str) -> list[str]:
    if not (isinstance(value, list) and all(isinstance(item, str) for item in value)):
        raise ValueError(f"{where} is not a list of strings")
    return value


def toml(text: str, where: str) -> dict:
    try:
        return tomllib.loads(text)
    except tomllib.TOMLDecodeError as error:
        raise ValueError(f"{where} is not valid TOML: {error}") from error


def lock_packages(text: str, where: str) -> dict[str, dict[str, str]]:
    """The packages of a Cargo.lock: name -> version -> source."""
    packages = toml(text, where).get("package", [])
    if not isinstance(packages, list):
        raise ValueError(f"{where} package is not an array of tables")
    found: dict[str, dict[str, str]] = {}
    for package in packages:
        fields = table(package, f"{where} [[package]]")
        name, version, source = fields.get("name"), fields.get("version"), fields.get("source", NO_SOURCE)
        if not all(isinstance(value, str) for value in (name, version, source)):
            raise ValueError(f"{where} holds a [[package]] without a name, a version and a source as text: {package!r}")
        found.setdefault(name, {})[version] = source
    return found


def compare_versions(name: str, old: dict[str, str], new: dict[str, str]) -> PackageDiff:
    """One package's versions before and after: one version swapped for another is a change, and
    so is a version that comes from another source; otherwise each version is added or removed."""
    gone, came = sorted(old.keys() - new.keys()), sorted(new.keys() - old.keys())
    moved = tuple((name, v, v, new[v]) for v in sorted(old.keys() & new.keys()) if old[v] != new[v])
    if len(gone) == len(came) == 1:
        return PackageDiff(changed=moved + ((name, gone[0], came[0], new[came[0]]),))
    return PackageDiff(added=tuple((name, "", v, new[v]) for v in came),
                       removed=tuple((name, v, "", old[v]) for v in gone), changed=moved)


def cargo_lock_diff(old: str, new: str, path: str = "Cargo.lock") -> PackageDiff:
    """The packages a Cargo.lock adds, removes and changes (an empty text holds none). When either
    side does not parse, the diff holds only that finding, which names `path`."""
    try:
        before = lock_packages(old, f"{path} before the change")
        after = lock_packages(new, f"{path} after the change")
    except ValueError as error:
        return PackageDiff(errors=(str(error),))
    names = before.keys() | after.keys()
    return total(compare_versions(name, before.get(name, {}), after.get(name, {})) for name in names)


def requirement(line: str) -> tuple[str, str]:
    """A requirement's normalized name and the rest of it (extras, version, markers or URL); a line
    that starts with no name (a bare URL or path) is its own name."""
    match = REQUIREMENT.fullmatch(line)
    if match is None:
        return line.strip(), ""
    return re.sub(r"[-_.]+", "-", match[1]).lower(), match[2].strip()


def requirements_lines(text: str) -> Iterator[str]:
    """The requirements of a requirements file: continuations joined; comments, option lines
    (`-r`, `--index-url`) and per-requirement options (`--hash`) left out."""
    for line in text.replace("\\\n", " ").splitlines():
        line = re.split(r"(?:^|\s)#", line, maxsplit=1)[0]
        line = re.split(r"(?:^|\s)-", line, maxsplit=1)[0].strip()
        if line:
            yield line


def group(groups: dict, name: str, where: str, seen: tuple[str, ...] = ()) -> Iterator[str]:
    """The requirements of a dependency group, every `include-group` resolved."""
    if name in seen or name not in groups:
        raise ValueError(f"{where} [dependency-groups] includes {name!r}, which is undefined or includes itself")
    for item in strings_and_tables(groups[name], f"{where} [dependency-groups] {name}"):
        if isinstance(item, str):
            yield item
        else:
            yield from group(groups, included(item, where, name), where, seen + (name,))


def strings_and_tables(value: object, where: str) -> list[str | dict]:
    if not (isinstance(value, list) and all(isinstance(item, str | dict) for item in value)):
        raise ValueError(f"{where} is not a list of requirements and include-group tables")
    return value


def included(item: dict, where: str, name: str) -> str:
    """The group an `{include-group = "x"}` table names."""
    if item.keys() != {"include-group"} or not isinstance(item["include-group"], str):
        raise ValueError(
            f"{where} [dependency-groups] {name} holds {item!r}, neither a requirement nor an include-group"
        )
    return item["include-group"]


def pyproject_lines(text: str, where: str) -> Iterator[str]:
    data = toml(text, where)
    project = table(data.get("project", {}), f"{where} [project]")
    yield from strings(project.get("dependencies", []), f"{where} [project] dependencies")
    extras = table(project.get("optional-dependencies", {}), f"{where} [project.optional-dependencies]")
    for extra, listed in extras.items():
        yield from strings(listed, f"{where} [project.optional-dependencies] {extra}")
    groups = table(data.get("dependency-groups", {}), f"{where} [dependency-groups]")
    for name in groups:
        yield from group(groups, name, where)
    build = table(data.get("build-system", {}), f"{where} [build-system]")
    yield from strings(build.get("requires", []), f"{where} [build-system] requires")


def declared(path: str, text: str, where: str) -> dict[str, str]:
    """The packages a dependency file declares: name -> its specs, sorted and joined by ` | `."""
    is_pyproject = PurePosixPath(path).name == "pyproject.toml"
    lines = pyproject_lines(text, where) if is_pyproject else requirements_lines(text)
    specs: dict[str, set[str]] = {}
    for line in lines:
        name, spec = requirement(line)
        specs.setdefault(name, set()).add(spec)
    return {name: " | ".join(sorted(found)) for name, found in specs.items()}


def compare_specs(path: str, old: dict[str, str], new: dict[str, str]) -> PackageDiff:
    return PackageDiff(
        added=tuple(sorted((n, "", new[n], path) for n in new.keys() - old.keys())),
        removed=tuple(sorted((n, old[n], "", path) for n in old.keys() - new.keys())),
        changed=tuple(sorted((n, old[n], new[n], path) for n in old.keys() & new.keys() if old[n] != new[n])),
    )


def file_diff(path: str, old: str, new: str) -> PackageDiff:
    try:
        before = declared(path, old, f"{path} before the change")
        after = declared(path, new, f"{path} after the change")
    except ValueError as error:
        return PackageDiff(errors=(str(error),))
    return compare_specs(path, before, after)


def declared_python_diff(old_files: dict[str, str], new_files: dict[str, str]) -> PackageDiff:
    """The Python packages the dependency files declare before and after (path -> text; a file
    absent on one side declares nothing there): `pyproject.toml` from `[project] dependencies`,
    `[project.optional-dependencies]`, `[dependency-groups]` and `[build-system] requires`,
    `requirements*.txt` line by line."""
    paths = old_files.keys() | new_files.keys()
    return total(file_diff(path, old_files.get(path, ""), new_files.get(path, "")) for path in paths)


def is_python_dependencies(path: str) -> bool:
    name = PurePosixPath(path).name
    return name == "pyproject.toml" or fnmatch(name, "requirements*.txt")


@dataclass(frozen=True)
class Texts:
    """The dependency files at one end of the diff: path -> text, and the ones that are not UTF-8."""

    files: dict[str, str]
    errors: tuple[str, ...]


def texts(root: Path, sha: str, paths: list[str], side: str) -> Texts:
    """The texts of `paths` at `sha`; a path absent there is left out."""
    listed = git_bytes(root, "ls-tree", "-r", "--name-only", "-z", sha, "--", *paths).split(b"\0")
    present = {os.fsdecode(path) for path in listed if path}
    files, errors = {}, []
    for path in sorted(present):
        try:
            files[path] = git_bytes(root, "show", f"{sha}:{path}").decode()
        except UnicodeDecodeError as error:
            errors.append(f"{path} {side} is not UTF-8 text: {error}")
    return Texts(files, tuple(errors))


def both_ends(change: Change, root: Path, paths: list[str]) -> tuple[Texts, Texts]:
    """The texts of `paths` where the diff starts (the merge base of base and head) and at the
    head."""
    start = git_text(root, "merge-base", "--end-of-options", change.base, change.head)
    return texts(root, start, paths, "before the change"), texts(root, change.head, paths, "after the change")


def dependency_diffs(change: Change, root: Path, paths: list[Touched]) -> tuple[PackageDiff, PackageDiff]:
    """The Cargo.lock and the Python dependency diffs of the touched dependency files."""
    locks = [t.path for t in paths if PurePosixPath(t.path).name == "Cargo.lock"]
    python = [t.path for t in paths if is_python_dependencies(t.path)]
    cargo, declared_diff = PackageDiff(), PackageDiff()
    if locks:
        old, new = both_ends(change, root, locks)
        cargo = total([*(cargo_lock_diff(old.files.get(p, ""), new.files.get(p, ""), p) for p in locks),
                       PackageDiff(errors=old.errors + new.errors)])
    if python:
        old, new = both_ends(change, root, python)
        declared_diff = total([declared_python_diff(old.files, new.files), PackageDiff(errors=old.errors + new.errors)])
    return cargo, declared_diff


def flagged(touched: list[Touched], cargo: PackageDiff, python: PackageDiff) -> bool:
    """Whether a change needs a confirmation: a path of any kind, a package added or changed, or a
    dependency file that could not be read."""
    return any(t.kinds for t in touched) or any(d.added or d.changed or d.errors for d in (cargo, python))
