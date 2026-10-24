"""Tests of the audit after a run, against a fake `docker` whose run-phase container writes into
the checkout (FAKE_RUN_WRITES)."""

import signal
import time
import unittest

from test_sandbox import World, mount_at

TRACKED = (".gitignore", "Makefile", "src/lib.rs", "crates/oxedi_py/python/oxedi/__init__.py")


class Audit(World):
    def test_audit_lists_and_exits_3(self):
        checkout = self.checkout("c", *TRACKED)
        writes = (
            "echo edit >> src/lib.rs && mkdir src/__pycache__ && echo pyc > src/__pycache__/x.cpython-313.pyc"
            " && echo so > crates/oxedi_py/python/oxedi/_core.abi3.so"
            f" && ln -s '{self.home / '.ssh'}' keys"
        )
        result = self.sandbox(checkout, FAKE_RUN_WRITES=writes)
        self.assertEqual(result.returncode, 3, result.stderr)
        self.assertIn("sandbox: the run changed tracked file src/lib.rs\n", result.stderr)
        self.assertIn(
            f"sandbox: the run left a new path keys, a symbolic link to {self.home / '.ssh'} outside the checkout\n",
            result.stderr,
        )
        self.assertIn(
            "sandbox: removed the caches and the built extension the run may have left: "
            "crates/oxedi_py/python/oxedi/_core.abi3.so, src/__pycache__\n",
            result.stderr,
        )
        self.assertIn(
            "sandbox: exit 3: the run changed the checkout (tracked files changed: 1, new paths: 1, not removed: 0); "
            "review them with git status and git diff before running anything from this checkout\n",
            result.stderr,
        )
        self.assertFalse((checkout / "src" / "__pycache__").exists())
        self.assertFalse((checkout / "crates/oxedi_py/python/oxedi/_core.abi3.so").exists())
        self.assertTrue((checkout / "keys").is_symlink())
        self.assertTrue((self.home / ".ssh").is_dir())

    def test_audit_clean_run(self):
        checkout = self.checkout("c", *TRACKED)
        writes = "mkdir -p target/debug .venv/lib && echo x > target/debug/x && echo x > .venv/lib/x"
        result = self.sandbox(checkout, FAKE_RUN_WRITES=writes, FAKE_RUN_EXIT="5")
        self.assertEqual(result.returncode, 5, result.stderr)
        self.assertNotIn("sandbox: the run", result.stderr)
        self.assertNotIn("sandbox: removed", result.stderr)
        self.assertNotIn("exit 3", result.stderr)

    def test_new_paths_outside_the_outputs_exit_3(self):
        checkout = self.checkout("c", *TRACKED)
        writes = "mkdir .claude .cargo && echo x > .claude/settings.json && echo x > .cargo/config.toml"
        result = self.sandbox(checkout, FAKE_RUN_WRITES=writes)
        self.assertEqual(result.returncode, 3, result.stderr)
        self.assertIn("sandbox: the run left a new path .cargo/config.toml\n", result.stderr)
        self.assertIn("sandbox: the run left a new path .claude/settings.json\n", result.stderr)
        self.assertNotIn("changed tracked file", result.stderr)

    def test_a_rewritten_gitignore_is_a_tracked_change(self):
        checkout = self.checkout("c", *TRACKED, ".venv-docs/bin/python")
        writes = "grep -v venv-docs .gitignore > g && cat g > .gitignore && rm g"
        result = self.sandbox(checkout, FAKE_RUN_WRITES=writes, FAKE_RUN_EXIT="1")
        self.assertEqual(mount_at(self.run_phase(), "/work/.venv-docs")["type"], "tmpfs")
        self.assertEqual(result.returncode, 3, result.stderr)
        self.assertIn("sandbox: the run changed tracked file .gitignore\n", result.stderr)
        # What the rewrite unignored shows as new already, before any later survey could unmask it.
        self.assertIn("sandbox: the run left a new path .venv-docs/bin/python\n", result.stderr)

    def test_a_tracked_file_turned_into_a_fifo_does_not_block(self):
        checkout = self.checkout("c", *TRACKED)
        result = self.sandbox(checkout, FAKE_RUN_WRITES="rm src/lib.rs && mkfifo src/lib.rs")
        self.assertEqual(result.returncode, 3, result.stderr)
        self.assertIn("sandbox: the run changed tracked file src/lib.rs\n", result.stderr)

    def test_an_output_that_cannot_be_removed_exits_3(self):
        checkout = self.checkout("c", *TRACKED)
        result = self.sandbox(checkout, FAKE_RUN_WRITES="mkdir src/__pycache__ && chmod 555 src")
        self.assertEqual(result.returncode, 3, result.stderr)
        self.assertIn(
            f"sandbox: cannot remove what the run left: [Errno 13] Permission denied: "
            f"'{checkout / 'src' / '__pycache__'}'; remove it, and any other Python cache or built extension "
            "in the checkout, by hand\n",
            result.stderr,
        )
        self.assertIn("not removed: 1", result.stderr)

    def test_audit_after_ctrl_c(self):
        checkout = self.checkout("c", *TRACKED)
        process = self.start(checkout, FAKE_RUN_WRITES="echo edit >> src/lib.rs", FAKE_RUN_SLEEP="30")
        deadline = time.monotonic() + 20
        while not (self.fake / "run.pid").exists() and time.monotonic() < deadline:
            time.sleep(0.05)
        process.send_signal(signal.SIGINT)
        _, err = process.communicate(timeout=20)
        self.assertEqual(process.returncode, 130)
        self.assertIn("sandbox: the run changed tracked file src/lib.rs\n", err)


if __name__ == "__main__":
    unittest.main()
