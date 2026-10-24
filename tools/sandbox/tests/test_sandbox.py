"""Tests of sandbox.py: the docker arguments of each phase, built from a surveyed checkout, and the
whole runner against a fake `docker` (fake_docker.py), so every refusal, error line and kill is
proven on the arguments docker would receive."""

import json
import os
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from itertools import pairwise
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCRIPT = HERE.parent / "sandbox.py"
REPO = HERE.parents[2]
sys.path.insert(0, str(SCRIPT.parent))
import sandbox  # noqa: E402

UID, GID = os.getuid(), os.getgid()
LIMITS = sandbox.Limits(cpus=3, memory_gb=4, timeout_s=60)
SECRETS = {
    "GH_TOKEN": "ghs_sandboxtestsecret1",
    "AWS_SECRET_ACCESS_KEY": "awssandboxtestsecret2",
    "PYPI_TOKEN": "pypi-sandboxtestsecret3",
}


def mounts(argv: list[str]) -> list[dict]:
    """Every `--mount` of an argument list, as a dict of its fields (`readonly` maps to "")."""
    found = []
    for flag, spec in pairwise(argv):
        if flag == "--mount":
            found.append(dict(part.partition("=")[::2] for part in spec.split(",")))
    return found


def mount_at(argv: list[str], destination: str) -> dict:
    matches = [m for m in mounts(argv) if m.get("dst") == destination]
    assert len(matches) == 1, f"{len(matches)} mounts at {destination} in {argv}"
    return matches[0]


def envs(argv: list[str]) -> list[str]:
    return [value for flag, value in pairwise(argv) if flag == "--env"]


def has_pair(argv: list[str], flag: str, value: str) -> bool:
    return any(a == flag and b == value for a, b in pairwise(argv))


def make_checkout(root: Path, *paths: str) -> Path:
    """A checkout holding `paths`: a trailing slash makes a directory, anything else a file."""
    root.mkdir(parents=True)
    for path in paths:
        target = root / path
        if path.endswith("/"):
            target.mkdir(parents=True, exist_ok=True)
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text("x\n")
    return root


FULL_SET = [
    "Makefile", "make/", "tools/", ".cargo/", ".github/", "rust-toolchain.toml", "Cargo.toml",
    "Cargo.lock", "CLAUDE.md", ".claude/", "crates/oxedi_py/pyproject.toml", ".git",
]


class World(unittest.TestCase):
    """A temporary home, a fake `docker` first on PATH and checkouts to run the sandbox over."""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.tmp = Path(self._tmp.name)
        self.home = self.tmp / "home"
        (self.home / ".ssh").mkdir(parents=True)
        self.fake = self.tmp / "fake"
        self.fake.mkdir()
        self.bin = self.tmp / "bin"
        self.bin.mkdir()
        shim = self.bin / "docker"
        shim.write_text(f'#!/bin/sh\nexec "{sys.executable}" "{HERE / "fake_docker.py"}" "$@"\n')
        shim.chmod(0o755)

    def tearDown(self):
        self._tmp.cleanup()

    def env(self, **extra: str) -> dict:
        env = {
            "PATH": f"{self.bin}{os.pathsep}{os.environ['PATH']}",
            "HOME": str(self.home),
            "FAKE_DIR": str(self.fake),
            "SANDBOX_CMD": "cargo test",
        }
        env.update(extra)
        return env

    def checkout(self, name: str = "checkout", *paths: str) -> Path:
        return make_checkout(self.tmp / name, *(paths or ("Makefile",)))

    def start(self, checkout: Path, **extra: str) -> subprocess.Popen:
        return subprocess.Popen(
            [sys.executable, str(SCRIPT), "--image", "oxedi-sandbox:test", "--checkout", str(checkout)],
            env=self.env(**extra), stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        )

    def sandbox(self, checkout: Path, **extra: str) -> subprocess.CompletedProcess:
        process = self.start(checkout, **extra)
        out, err = process.communicate(timeout=60)
        return subprocess.CompletedProcess(process.args, process.returncode, out, err)

    def calls(self) -> list[dict]:
        log = self.fake / "calls.jsonl"
        if not log.exists():
            return []
        return [json.loads(line) for line in log.read_text().splitlines()]

    def runs(self) -> list[dict]:
        return [call for call in self.calls() if call["argv"][:1] == ["run"]]

    def run_phase(self) -> list[str]:
        runs = [call["argv"] for call in self.runs() if "bash" in call["argv"]]
        self.assertEqual(len(runs), 1, self.calls())
        return runs[0]

    def phase_args(self, phase, checkout: Path) -> list[str]:
        surveyed = sandbox.survey(checkout)
        storage = sandbox.storage_for(self.home / ".cache" / "oxedi-sandbox", surveyed)
        return sandbox.docker_args(phase, surveyed, UID, GID, sandbox.Runtime.RUNC, LIMITS, storage)


class Arguments(World):
    def test_both_phases_hardened(self):
        checkout = self.checkout("c", "Makefile", ".cargo/config.toml")
        name = f"oxedi-sandbox-{sandbox.path_hash(checkout)}"
        cache = self.home / ".cache" / "oxedi-sandbox"
        for phase in sandbox.Phase:
            with self.subTest(phase=phase):
                argv = self.phase_args(phase, checkout)
                for flag in ("--rm", "--init", "--read-only"):
                    self.assertIn(flag, argv)
                for flag, value in [
                    ("--name", name), ("--user", f"{UID}:{GID}"), ("--cap-drop", "ALL"),
                    ("--security-opt", "no-new-privileges"), ("--pids-limit", "4096"),
                    ("--memory", "4g"), ("--memory-swap", "4g"), ("--cpus", "3"),
                    ("--tmpfs", "/tmp:rw,nosuid,nodev,size=2g"), ("--pull", "never"),
                ]:
                    self.assertTrue(has_pair(argv, flag, value), f"{flag} {value} missing in {argv}")
                self.assertIn("HOME=/tmp/home", envs(argv))
                self.assertFalse(any("seccomp" in a or "privileged" in a or "--cap-add" == a for a in argv))
                self.assertFalse(any(a in ("-e", "-v", "--volume", "--env-file") for a in argv))
        fetch = self.phase_args(sandbox.Phase.FETCH, checkout)
        self.assertNotIn("--network", fetch)
        self.assertIn("readonly", mount_at(fetch, "/work"))
        self.assertEqual(mount_at(fetch, "/work/.cargo")["type"], "tmpfs")
        self.assertIn("readonly", mount_at(fetch, "/work/.cargo"))
        cargo = [m for m in mounts(fetch) if m.get("src") == str(cache / "cargo")]
        uv = [m for m in mounts(fetch) if m.get("src") == str(cache / "uv")]
        self.assertEqual(len(cargo), 1)
        self.assertEqual(len(uv), 1)
        self.assertNotIn("readonly", cargo[0])
        self.assertNotIn("readonly", uv[0])
        self.assertIn("CARGO_BUILD_RUSTC_WRAPPER=", envs(fetch))
        self.assertIn("CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER=", envs(fetch))
        self.assertIn("UV_NO_CONFIG=1", envs(fetch))
        self.assertFalse([m for m in mounts(fetch) if m.get("dst") == "/work/target"])
        self.assertIn(f"UV_CACHE_DIR={sandbox.UV_CACHE}", envs(fetch))
        self.assertIn(f"CARGO_HOME={sandbox.CARGO_HOME}", envs(fetch))
        run = self.phase_args(sandbox.Phase.RUN, checkout)
        self.assertTrue(has_pair(run, "--network", "none"))
        self.assertNotIn("readonly", mount_at(run, "/work"))
        cargo = [m for m in mounts(run) if m.get("src") == str(cache / "cargo")]
        self.assertEqual(len(cargo), 1)
        self.assertIn("readonly", cargo[0])
        self.assertFalse([m for m in mounts(run) if m.get("src") == str(cache / "uv")])
        for variable in ("UV_CACHE_DIR=/tmp/uv-cache", "CARGO_NET_OFFLINE=true", "UV_OFFLINE=1", "CARGO_INCREMENTAL=0"):
            self.assertIn(variable, envs(run))

    def test_host_executed_set_read_only(self):
        checkout = self.checkout("c", *FULL_SET, ".doc/scratch/notes.md", ".workflow-kit/x/brief.md")
        for phase in sandbox.Phase:
            argv = self.phase_args(phase, checkout)
            for path in (p.rstrip("/") for p in FULL_SET):
                with self.subTest(phase=phase, path=path):
                    mount = mount_at(argv, f"/work/{path}")
                    self.assertIn("readonly", mount)
                    if phase is sandbox.Phase.FETCH and path == ".cargo":
                        self.assertEqual(mount["type"], "tmpfs")
                    else:
                        self.assertEqual((mount["type"], mount["src"]), ("bind", str(checkout / path)))
            for path in (".doc", ".workflow-kit"):
                with self.subTest(phase=phase, masked=path):
                    mount = mount_at(argv, f"/work/{path}")
                    self.assertEqual(mount["type"], "tmpfs")
                    self.assertIn("readonly", mount)
            self.assertFalse([m for m in mounts(argv) if m.get("dst") == "/work/.doc/scratch"])

    def test_masks_only_what_exists(self):
        checkout = self.checkout("c", "Makefile")
        for phase in sandbox.Phase:
            destinations = {m.get("dst") for m in mounts(self.phase_args(phase, checkout))}
            self.assertFalse(destinations & {"/work/.doc", "/work/.workflow-kit", "/work/.cargo", "/work/tools"})

    def test_symlink_in_read_only_set_refused(self):
        checkout = self.checkout("c", "Makefile")
        (checkout / "tools").symlink_to(self.home / ".ssh")
        result = self.sandbox(checkout)
        self.assertEqual(result.returncode, 2)
        self.assertIn("sandbox: refused: tools is a symbolic link", result.stderr)
        self.assertFalse(self.runs())


class Runner(World):
    def test_host_secrets_never_pass(self):
        checkout = self.checkout()
        result = self.sandbox(checkout, **SECRETS)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(self.calls())
        for call in self.calls():
            for argument in call["argv"]:
                for name, value in SECRETS.items():
                    self.assertNotIn(value, argument)
                    self.assertNotIn(name, argument)
        for call in self.runs():
            for mount in mounts(call["argv"]):
                source = mount.get("src", "")
                self.assertNotEqual(source, str(self.home))
                self.assertFalse(source.startswith(str(self.home / ".ssh")))
                self.assertNotIn("docker.sock", source)

    def test_cmd_through_make(self):
        command = 'cargo test -p oxedi_core --lib value:: && echo "a b; c" $$HOME'
        result = subprocess.run(
            ["make", "--no-print-directory", "-C", str(REPO), "sandbox", f"CMD={command}"],
            env=self.env(), capture_output=True, text=True, check=False, timeout=120,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        argv = self.run_phase()
        self.assertEqual(argv[-3:-1], ["bash", "-c"])
        self.assertEqual(argv[-1], 'mkdir -p "$HOME" && cargo test -p oxedi_core --lib value:: && echo "a b; c" $HOME')

    def test_image_built_when_missing(self):
        result = subprocess.run(
            ["make", "--no-print-directory", "-C", str(REPO), "sandbox", "CMD=true"],
            env=self.env(FAKE_NO_IMAGE="1"), capture_output=True, text=True, check=False, timeout=120,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        builds = [c["argv"] for c in self.calls() if c["argv"][:1] == ["build"]]
        self.assertEqual(len(builds), 2)
        self.assertTrue(any(a.startswith("oxedi.sandbox.apt=") for a in builds[1]))
        helper = [c["argv"] for c in self.runs() if "cat" in c["argv"]]
        self.assertEqual(len(helper), 1)
        self.assertTrue(has_pair(helper[0], "--user", "65534:65534"))
        self.assertTrue(has_pair(helper[0], "--security-opt", "no-new-privileges"))
        self.run_phase()

    def test_cmd_required(self):
        result = self.sandbox(self.checkout(), SANDBOX_CMD="")
        self.assertEqual(result.returncode, 2)
        self.assertIn('sandbox: no command: set CMD, for example make sandbox CMD="make gates"', result.stderr)

    def test_storage_per_checkout(self):
        first, second = self.checkout("a"), self.checkout("b")
        for checkout in (first, second):
            self.assertEqual(self.sandbox(checkout).returncode, 0)
        cache = self.home / ".cache" / "oxedi-sandbox"
        runs = [c for c in self.runs() if "bash" in c["argv"]]
        self.assertEqual(len(runs), 2)
        targets = [mount_at(c["argv"], "/work/target")["src"] for c in runs]
        venvs = [mount_at(c["argv"], "/work/.venv")["src"] for c in runs]
        self.assertEqual(targets, [str(cache / sandbox.path_hash(c) / "target") for c in (first, second)])
        self.assertEqual(venvs, [str(cache / sandbox.path_hash(c) / "venv") for c in (first, second)])
        self.assertNotEqual(targets[0], targets[1])
        cargo = {m["src"] for c in runs for m in mounts(c["argv"]) if m.get("dst") == sandbox.CARGO_HOME}
        self.assertEqual(cargo, {str(cache / "cargo")})
        self.assertTrue((cache / "uv").is_dir())
        for call in runs:
            for source, state in call["binds"].items():
                with self.subTest(source=source):
                    self.assertEqual((state["exists"], state["uid"]), (True, UID))
        for checkout in (first, second):
            self.assertTrue((checkout / "target").is_dir())
            self.assertTrue((checkout / ".venv").is_dir())

    def test_xdg_cache_home_holds_the_storage(self):
        checkout = self.checkout()
        xdg = self.tmp / "xdg"
        self.assertEqual(self.sandbox(checkout, XDG_CACHE_HOME=str(xdg)).returncode, 0)
        target = mount_at(self.run_phase(), "/work/target")["src"]
        self.assertEqual(target, str(xdg / "oxedi-sandbox" / sandbox.path_hash(checkout) / "target"))

    def test_runsc_when_listed(self):
        checkout = self.checkout()
        info = {"SecurityOptions": [], "Runtimes": {"runc": {}, "runsc": {}}, "NCPU": 4, "MemTotal": 8 << 30}
        result = self.sandbox(checkout, FAKE_INFO=json.dumps(info))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(has_pair(self.run_phase(), "--runtime", "runsc"))
        self.assertIn("runtime runsc", result.stderr)

    def test_runc_otherwise(self):
        result = self.sandbox(self.checkout())
        self.assertNotIn("--runtime", self.run_phase())
        self.assertIn("runtime runc", result.stderr)

    def test_rootless_refused(self):
        info = {"SecurityOptions": ["name=seccomp,profile=builtin", "name=rootless"], "Runtimes": {"runc": {}},
                "NCPU": 4, "MemTotal": 8 << 30}
        result = self.sandbox(self.checkout(), FAKE_INFO=json.dumps(info))
        self.assertEqual(result.returncode, 2)
        self.assertEqual(
            result.stderr,
            "sandbox: refused: the Docker daemon is rootless (name=rootless in docker info); "
            "use a rootful Docker Engine or Docker Desktop\n",
        )
        self.assertFalse(self.runs())

    def test_limits_default_from_the_daemon(self):
        result = self.sandbox(self.checkout())
        argv = self.run_phase()
        self.assertTrue(has_pair(argv, "--cpus", "15"))
        self.assertTrue(has_pair(argv, "--memory", "12g"))
        self.assertIn("15 CPUs, 12 GB, 3600 s", result.stderr)

    def test_limits_from_the_knobs(self):
        result = self.sandbox(self.checkout(), SANDBOX_CPUS="2", SANDBOX_MEMORY="5", SANDBOX_TIMEOUT="30")
        argv = self.run_phase()
        self.assertTrue(has_pair(argv, "--cpus", "2"))
        self.assertTrue(has_pair(argv, "--memory-swap", "5g"))
        self.assertIn("2 CPUs, 5 GB, 30 s", result.stderr)

    def test_knob_not_a_positive_integer_refused(self):
        for value in ("abc", "0", "-1", "1.5", "\u00b2"):
            with self.subTest(value=value):
                result = self.sandbox(self.checkout(f"c{value}"), SANDBOX_MEMORY=value)
                self.assertEqual(result.returncode, 2)
                self.assertIn(f'sandbox: SANDBOX_MEMORY must be a positive whole number of GB, got "{value}"',
                              result.stderr)

    def test_already_running_refused(self):
        checkout = self.checkout()
        name = f"oxedi-sandbox-{sandbox.path_hash(checkout)}"
        result = self.sandbox(checkout, FAKE_RUNNING=name)
        self.assertEqual(result.returncode, 2)
        self.assertIn(f"sandbox: {name} is already running for this checkout", result.stderr)
        self.assertIn(f"docker kill {name}", result.stderr)
        self.assertFalse(self.runs())

    def test_docker_missing_explained(self):
        env = self.env()
        self.bin.joinpath("docker").unlink()
        env["PATH"] = str(self.bin)
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "--image", "oxedi-sandbox:test", "--checkout", str(self.checkout())],
            env=env, capture_output=True, text=True, check=False,
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("sandbox: docker was not found on PATH", result.stderr)

    def test_missing_image_explained(self):
        result = self.sandbox(self.checkout(), FAKE_NO_IMAGE="1")
        self.assertEqual(result.returncode, 2)
        self.assertIn("sandbox: image oxedi-sandbox:test not found; build it with make sandbox-image", result.stderr)

    def test_start_failure_distinct(self):
        cause = "docker: Error response from daemon: invalid mount config for type bind"
        result = self.sandbox(self.checkout(), FAKE_RUN_EXIT="125", FAKE_RUN_STDERR=cause + "\n")
        self.assertEqual(result.returncode, 125)
        self.assertIn(f"sandbox: docker could not start the container: {cause}", result.stderr)
        self.assertNotIn("command exited", result.stderr)

    def test_command_status_passes_through(self):
        result = self.sandbox(self.checkout(), FAKE_RUN_EXIT="3")
        self.assertEqual(result.returncode, 3)
        self.assertIn("sandbox: command exited 3", result.stderr)

    def test_oom_hint(self):
        result = self.sandbox(self.checkout(), FAKE_RUN_EXIT="137", SANDBOX_MEMORY="6")
        self.assertEqual(result.returncode, 137)
        self.assertIn(
            "sandbox: the container was killed (exit 137), most likely by its 6 GB memory limit; "
            "raise it with SANDBOX_MEMORY=<GB>",
            result.stderr,
        )

    def test_timeout_kills(self):
        checkout = self.checkout()
        started = time.monotonic()
        result = self.sandbox(checkout, FAKE_RUN_SLEEP="30", SANDBOX_TIMEOUT="1")
        self.assertLess(time.monotonic() - started, 20)
        self.assertEqual(result.returncode, 124)
        self.assertIn("sandbox: the command ran past SANDBOX_TIMEOUT=1 s; the container was killed", result.stderr)
        name = f"oxedi-sandbox-{sandbox.path_hash(checkout)}"
        self.assertIn(["kill", name], [c["argv"] for c in self.calls()])

    def test_ctrl_c_kills(self):
        checkout = self.checkout()
        process = self.start(checkout, FAKE_RUN_SLEEP="30")
        deadline = time.monotonic() + 20
        while not (self.fake / "run.pid").exists() and time.monotonic() < deadline:
            time.sleep(0.05)
        process.send_signal(signal.SIGINT)
        _, err = process.communicate(timeout=20)
        self.assertEqual(process.returncode, 130)
        self.assertIn("sandbox: interrupted; the container, if it had started, was killed", err)
        name = f"oxedi-sandbox-{sandbox.path_hash(checkout)}"
        self.assertIn(["kill", name], [c["argv"] for c in self.calls()])

    def test_no_kill_after_a_normal_exit(self):
        self.assertEqual(self.sandbox(self.checkout()).returncode, 0)
        self.assertFalse([c for c in self.calls() if c["argv"][:1] == ["kill"]])


def command_of(argv: list[str]) -> list[str]:
    return argv[argv.index("oxedi-sandbox:test") + 1:]


CARGO_FETCH = ["cargo", "fetch", "--locked"]
UV_VENV = ["uv", "venv", "--managed-python", "--python", "3.13", "/work/.venv"]
UV_INSTALL = [
    "uv", "pip", "install", "--python", "/work/.venv/bin/python", "--no-build",
    "--group", "crates/oxedi_py/pyproject.toml:dev",
]


class Fetch(World):
    def fetches(self) -> list[list[str]]:
        return [c["argv"] for c in self.runs() if "bash" not in c["argv"]]

    def venv(self, checkout: Path) -> Path:
        return self.home / ".cache" / "oxedi-sandbox" / sandbox.path_hash(checkout) / "venv"

    def test_fetch_commands_fixed(self):
        self.assertEqual(sandbox.fetch_commands(), [CARGO_FETCH, UV_VENV, UV_INSTALL])

    def test_what_a_run_leaves_never_reaches_the_next_fetch(self):
        checkout = self.checkout()
        self.assertEqual(self.sandbox(checkout).returncode, 0)
        clean = self.fetches()
        venv = self.venv(checkout)
        outside = self.tmp / "outside"
        (outside / "keep").mkdir(parents=True)
        site = venv / "lib" / "python3.13" / "site-packages"
        site.mkdir(parents=True)
        (site / "evil.pth").write_text("import os; os.system('touch /work/marker')\n")
        (venv / "bin").mkdir()
        (venv / "bin" / "python").symlink_to(outside / "keep")
        (venv / "outside-link").symlink_to(outside)
        (venv / "pyvenv.cfg").write_text("version_info = 3.13\n")
        (checkout / "uv.toml").write_text('index-url = "https://attacker.example/simple"\n')
        (checkout / ".python-version").write_text("3.11\n")
        (self.fake / "calls.jsonl").unlink()
        self.assertEqual(self.sandbox(checkout).returncode, 0)
        planted = self.fetches()
        self.assertEqual(planted, clean)
        for argv in planted:
            self.assertIn("UV_NO_CONFIG=1", envs(argv))
            self.assertFalse([m for m in mounts(argv) if m.get("dst") == "/work/target"])
            self.assertIn("readonly", mount_at(argv, "/work"))
        first = [c for c in self.runs() if "bash" not in c["argv"]][0]
        self.assertEqual(first["binds"][str(venv)]["entries"], [])
        self.assertTrue(venv.is_dir())
        self.assertTrue((outside / "keep").is_dir())

    def test_fetch_failure_stops_without_offline_fallback(self):
        result = self.sandbox(self.checkout(), FAKE_FETCH_EXIT="101")
        self.assertEqual(result.returncode, 101)
        self.assertEqual(len(self.fetches()), 1)
        self.assertFalse([c for c in self.runs() if "bash" in c["argv"]])
        self.assertIn(
            "sandbox: fetch failed: cargo fetch --locked exited 101; the fetch needs the network "
            "and never falls back to the caches offline",
            result.stderr,
        )


if __name__ == "__main__":
    unittest.main()
