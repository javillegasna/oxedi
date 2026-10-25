"""Tests of risk.py: the kinds of the paths a change touches, the Cargo.lock and Python dependency
diffs, and when a change is flagged, on temporary repositories only."""

import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import risk  # noqa: E402
from risk import Kind, PackageDiff, Touched  # noqa: E402
from source import Change  # noqa: E402
from test_source import ISOLATED, git, new_repo  # noqa: E402

REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
GIT_SOURCE = "git+https://github.com/example/c?rev=abc#abc0123"
PYPROJECT = "crates/oxedi_py/pyproject.toml"

OLD_LOCK = f"""version = 3

[[package]]
name = "a"
version = "1.0.0"
source = "{REGISTRY}"

[[package]]
name = "b"
version = "0.1.0"
source = "{REGISTRY}"

[[package]]
name = "syn"
version = "1.0.109"
source = "{REGISTRY}"

[[package]]
name = "syn"
version = "2.0.0"
source = "{REGISTRY}"

[[package]]
name = "oxedi_core"
version = "0.6.0"
"""
NEW_LOCK = f"""version = 3

[[package]]
name = "a"
version = "1.1.0"
source = "{REGISTRY}"

[[package]]
name = "c"
version = "2.0.0"
source = "{GIT_SOURCE}"

[[package]]
name = "syn"
version = "2.0.0"
source = "{REGISTRY}"

[[package]]
name = "oxedi_core"
version = "0.6.0"
"""


def pyproject(arrow: str, build: str = '"maturin>=1.5"') -> str:
    return f"""[project]
name = "oxedi"
dependencies = []

[project.optional-dependencies]
pandas = ["pandas>=2"]

[dependency-groups]
dev = [{{include-group = "arrow"}}, "pytest>=8"]
arrow = ["{arrow}"]

[build-system]
requires = [{build}]
"""


class Repo(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.tmp = Path(self._tmp.name)
        isolated = mock.patch.dict(os.environ, ISOLATED)
        isolated.start()
        self.addCleanup(isolated.stop)
        self.repo = new_repo(self.tmp / "repo")

    def write(self, files: dict[str, str | bytes]) -> None:
        for path, content in files.items():
            target = self.repo / path
            target.parent.mkdir(parents=True, exist_ok=True)
            if isinstance(content, bytes):
                target.write_bytes(content)
            else:
                target.write_text(content)

    def commit(self, files: dict[str, str | bytes]) -> str:
        self.write(files)
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-q", "--allow-empty", "-m", "c")
        return git(self.repo, "rev-parse", "HEAD")

    def change(self, base: str, head: str) -> Change:
        return Change("ref", "ref-x", head, base, head, False)


class Kinds(Repo):
    def test_host_executed_reason_says_the_change_runs_inside(self):
        self.assertEqual(
            Kind.HOST_EXECUTED.why,
            "a file the host executes or trusts after a run: the checkout's copy, this change's, is bound "
            "read-only and runs inside the sandbox (only the sandbox runner, this path list and deny.toml "
            "are always the host's)",
        )

    def test_kinds_table(self):
        base = self.commit({"Makefile": "all:\n", "crates/oxedi_core/src/value/parse.rs": "fn a() {}\n"})
        expected = {
            "crates/oxedi_core/build.rs": (Kind.BUILD_SCRIPT,),
            "crates/x/Cargo.toml": (Kind.MANIFEST,),
            "crates/m/Cargo.toml": (Kind.MANIFEST, Kind.PROC_MACRO),
            "rust-toolchain.toml": (Kind.MANIFEST,),
            "requirements-dev.txt": (Kind.MANIFEST,),
            "make/sandbox.mk": (Kind.HOST_EXECUTED,),
            "tools/sandbox/deny.toml": (Kind.HOST_EXECUTED,),
            "tools/sandbox/risk.py": (Kind.HOST_EXECUTED, Kind.SCRIPT),
            ".github/scripts/lint.sh": (Kind.HOST_EXECUTED, Kind.SCRIPT),
            "crates/x/.cargo/config.toml": (Kind.HOST_EXECUTED,),
            "viewer/app.js": (Kind.SCRIPT,),
            "crates/oxedi_py/tests/conftest.py": (Kind.SCRIPT, Kind.TEST_HOOK),
            "docs/python/index.md": (Kind.RUN_DOCS,),
            "README.md": (Kind.RUN_DOCS,),
            "assets/logo.bin": (Kind.BINARY,),
            "link": (Kind.SYMLINK,),
            "vendor/sub": (Kind.SUBMODULE,),
            "crates/oxedi_core/src/value/parse.rs": (),
            "crates/oxedi_py/python/oxedi/__init__.py": (),
            "Makefile": (Kind.HOST_EXECUTED,),
        }
        files = {path: "x\n" for path in expected if path not in ("link", "vendor/sub", "Makefile")}
        files["crates/m/Cargo.toml"] = '[package]\nname = "m"\n\n[lib]\nproc-macro = true\n'
        files["assets/logo.bin"] = b"\0\1\2\3"
        files["crates/oxedi_core/src/value/parse.rs"] = "fn b() {}\n"
        self.write(files)
        (self.repo / "Makefile").unlink()
        os.symlink("README.md", self.repo / "link")
        git(self.repo, "add", "-A")
        git(self.repo, "update-index", "--add", "--cacheinfo", f"160000,{base},vendor/sub")
        git(self.repo, "commit", "-q", "-m", "head")
        head = git(self.repo, "rev-parse", "HEAD")
        found = risk.touched(self.change(base, head), self.repo)
        self.assertEqual({t.path: t.kinds for t in found}, expected)
        self.assertEqual([t.path for t in found], sorted(expected))
        statuses = {t.path: t.status for t in found}
        self.assertEqual(statuses["Makefile"], "D")
        self.assertEqual(statuses["crates/oxedi_core/src/value/parse.rs"], "M")
        self.assertEqual(statuses["crates/oxedi_core/build.rs"], "A")

    def test_the_change_attributes_cannot_hide_a_binary(self):
        base = self.commit({"a.txt": "a\n"})
        head = self.commit({"x.bin": b"\0\1\2\3", ".gitattributes": "*.bin diff\n"})
        self.assertEqual(git(self.repo, "rev-parse", "HEAD"), head)  # the worktree holds the change
        found = risk.touched(self.change(base, head), self.repo)
        self.assertEqual({t.path: t.kinds for t in found},
                         {"x.bin": (Kind.BINARY,), ".gitattributes": (Kind.GIT_ATTRIBUTES,)})

    def test_every_kind_says_why(self):
        for kind in Kind:
            with self.subTest(kind=kind):
                self.assertTrue(kind.why and "\n" not in kind.why)


class Cargo(unittest.TestCase):
    def test_cargo_lock_added_changed_removed(self):
        self.assertEqual(risk.cargo_lock_diff(OLD_LOCK, NEW_LOCK), PackageDiff(
            added=(("c", "", "2.0.0", GIT_SOURCE),),
            removed=(("b", "0.1.0", "", REGISTRY), ("syn", "1.0.109", "", REGISTRY)),
            changed=(("a", "1.0.0", "1.1.0", REGISTRY),),
        ))

    def test_the_same_version_from_a_second_source_is_added(self):
        evil = "git+https://evil/x#1"
        new = OLD_LOCK.replace('[[package]]\nname = "syn"\nversion = "2.0.0"',
                               f'[[package]]\nname = "syn"\nversion = "2.0.0"\nsource = "{evil}"\n\n'
                               '[[package]]\nname = "syn"\nversion = "2.0.0"')
        self.assertEqual(risk.cargo_lock_diff(OLD_LOCK, new), PackageDiff(added=(("syn", "", "2.0.0", evil),)))

    def test_a_version_from_another_source_is_a_change(self):
        new = OLD_LOCK.replace(f'version = "0.1.0"\nsource = "{REGISTRY}"', f'version = "0.1.0"\nsource = "{GIT_SOURCE}"')
        self.assertEqual(risk.cargo_lock_diff(OLD_LOCK, new), PackageDiff(changed=(("b", "0.1.0", "0.1.0", GIT_SOURCE),)))

    def test_a_new_lock_adds_every_package(self):
        diff = risk.cargo_lock_diff("", NEW_LOCK, "crates/x/Cargo.lock")
        self.assertEqual(diff.added, (("a", "", "1.1.0", REGISTRY), ("c", "", "2.0.0", GIT_SOURCE),
                                      ("oxedi_core", "", "0.6.0", "local"), ("syn", "", "2.0.0", REGISTRY)))

    def test_unparseable_lock_is_flagged(self):
        diff = risk.cargo_lock_diff(OLD_LOCK, "garbage [[")
        self.assertEqual(diff, PackageDiff(errors=(
            "Cargo.lock after the change is not valid TOML: Expected '=' after a key in a key/value pair "
            "(at line 1, column 9)",
        )))
        self.assertIs(risk.flagged([], diff, PackageDiff()), True)

    def test_a_package_without_a_version_is_a_finding(self):
        diff = risk.cargo_lock_diff(OLD_LOCK, '[[package]]\nname = "a"\n')
        self.assertEqual(diff.errors, (
            "Cargo.lock after the change holds a [[package]] without a name, a version and a source as text: "
            "{'name': 'a'}",
        ))


class Python(unittest.TestCase):
    def test_python_declared_include_group(self):
        diff = risk.declared_python_diff({PYPROJECT: pyproject("pyarrow>=14")}, {PYPROJECT: pyproject("pyarrow>=15")})
        self.assertEqual(diff, PackageDiff(changed=(("pyarrow", ">=14", ">=15", PYPROJECT),)))

    def test_every_table_counts(self):
        new = pyproject("pyarrow>=14", build='"maturin>=1.5", "setuptools"')
        new = new.replace('pandas = ["pandas>=2"]', 'pandas = ["pandas>=2", "Typing_Extensions>=4"]')
        diff = risk.declared_python_diff({PYPROJECT: pyproject("pyarrow>=14")}, {PYPROJECT: new})
        self.assertEqual(diff.added, (("setuptools", "", "", PYPROJECT), ("typing-extensions", "", ">=4", PYPROJECT)))

    def test_an_undefined_or_circular_group_is_a_finding(self):
        for groups, name in (('dev = [{include-group = "nope"}]', "'nope'"),
                             ('dev = [{include-group = "dev"}]', "'dev'")):
            with self.subTest(groups=groups):
                diff = risk.declared_python_diff({}, {PYPROJECT: f"[dependency-groups]\n{groups}\n"})
                self.assertEqual(diff.errors, (
                    f"{PYPROJECT} after the change [dependency-groups] includes {name}, "
                    "which is undefined or includes itself",
                ))

    def test_requirements_line_by_line(self):
        old = "# tools\nrequests==2.31  # http\n-r base.txt\nnumpy>=1 \\\n  --hash=sha256:ab\n"
        new = "requests==2.32\n-r base.txt\nrich ; python_version >= '3.11'\nhttps://example.com/x-1.0.whl\n"
        self.assertEqual(risk.declared_python_diff({"requirements.txt": old}, {"requirements.txt": new}), PackageDiff(
            added=(("https://example.com/x-1.0.whl", "", "", "requirements.txt"),
                   ("rich", "", "; python_version >= '3.11'", "requirements.txt")),
            removed=(("numpy", ">=1", "", "requirements.txt"),),
            changed=(("requests", "==2.31", "==2.32", "requirements.txt"),),
        ))

    def test_option_lines_are_declared_sources(self):
        new = ("-e git+https://evil/x#egg=x\n-r  base.txt\n-c constraints.txt\n"
               "--index-url https://user:s3cret@pypi.example/simple\nevil @ https://tok@example.com/e.whl\n")
        diff = risk.declared_python_diff({}, {"requirements.txt": new})
        self.assertEqual(diff.added, (
            ("--index-url https://<redacted>@pypi.example/simple", "", "", "requirements.txt"),
            ("-c constraints.txt", "", "", "requirements.txt"),
            ("-e git+https://evil/x#egg=x", "", "", "requirements.txt"),
            ("-r base.txt", "", "", "requirements.txt"),
            ("evil", "", "@ https://<redacted>@example.com/e.whl", "requirements.txt"),
        ))
        self.assertIs(risk.flagged([], PackageDiff(), diff), True)

    def test_a_malformed_pyproject_is_a_finding(self):
        diff = risk.declared_python_diff({}, {PYPROJECT: "[project]\ndependencies = \"pyarrow\"\n"})
        self.assertEqual(diff.errors, (f"{PYPROJECT} after the change [project] dependencies is not a list of strings",))


class Flagged(unittest.TestCase):
    def test_flagged(self):
        plain = [Touched("crates/oxedi_core/src/lib.rs", (), "M")]
        self.assertIs(risk.flagged(plain, PackageDiff(), PackageDiff()), False)
        removed_only = PackageDiff(removed=(("b", "0.1.0", "", REGISTRY),))
        self.assertIs(risk.flagged(plain, removed_only, PackageDiff()), False)
        self.assertIs(risk.flagged([Touched("build.rs", (Kind.BUILD_SCRIPT,), "A")], PackageDiff(), PackageDiff()), True)
        self.assertIs(risk.flagged(plain, PackageDiff(), PackageDiff(added=(("x", "", "", PYPROJECT),))), True)
        self.assertIs(risk.flagged(plain, PackageDiff(changed=(("a", "1", "2", REGISTRY),)), PackageDiff()), True)


class ImageHint(unittest.TestCase):
    def test_image_hint_table(self):
        hint = "this change touches the viewer; rerun with IMAGE=viewer to run its tests"
        base, viewer = risk.ImageKind.BASE, risk.ImageKind.VIEWER
        for path, kind, expected in (
            ("viewer/app.js", base, hint),
            ("crates/oxedi_wasm/src/lib.rs", base, hint),
            ("viewer/app.js", viewer, ""),
            ("crates/oxedi_wasm/src/lib.rs", viewer, ""),
            ("crates/oxedi_core/src/lib.rs", base, ""),
            ("crates/oxedi_wasmish/src/lib.rs", base, ""),
            ("docs/viewer/index.md", base, ""),
        ):
            with self.subTest(path=path, kind=kind):
                self.assertEqual(risk.image_hint([Touched(path, (), "M")], kind), expected)
        mixed = [Touched("crates/oxedi_core/src/lib.rs", (), "M"), Touched("viewer/app.js", (), "M")]
        self.assertEqual(risk.image_hint(mixed, base), hint)
        self.assertEqual(risk.image_hint([], base), "")


class Dependencies(Repo):
    def test_dependency_diffs_read_both_ends(self):
        base = self.commit({"Cargo.lock": OLD_LOCK, PYPROJECT: pyproject("pyarrow>=14"), "src/lib.rs": "x\n"})
        head = self.commit({"Cargo.lock": NEW_LOCK, PYPROJECT: pyproject("pyarrow>=15"),
                            "requirements-docs.txt": "mkdocs==1.6\n", "src/lib.rs": "y\n"})
        change = self.change(base, head)
        cargo, python = risk.dependency_diffs(change, self.repo, risk.touched(change, self.repo))
        self.assertEqual(cargo, risk.cargo_lock_diff(OLD_LOCK, NEW_LOCK))
        self.assertEqual(python, PackageDiff(
            added=(("mkdocs", "", "==1.6", "requirements-docs.txt"),),
            changed=(("pyarrow", ">=14", ">=15", PYPROJECT),),
        ))

    def test_a_lock_that_is_not_utf8_is_a_finding(self):
        base = self.commit({"Cargo.lock": OLD_LOCK})
        head = self.commit({"Cargo.lock": b"\xff\xfe"})
        change = self.change(base, head)
        cargo, python = risk.dependency_diffs(change, self.repo, risk.touched(change, self.repo))
        self.assertEqual(len(cargo.errors), 1)
        self.assertTrue(cargo.errors[0].startswith("Cargo.lock after the change is not UTF-8 text: "), cargo.errors)
        self.assertEqual(python, PackageDiff())


if __name__ == "__main__":
    unittest.main()
