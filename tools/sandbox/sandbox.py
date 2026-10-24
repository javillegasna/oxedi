#!/usr/bin/env python3
"""Runs a command of this checkout in a throwaway, hardened container of the sandbox image.

The container runs as the user, with a read-only root, no capabilities, no new privileges, the
default seccomp profile and limits on processes, memory and CPUs. It sees the checkout at /work,
with the files the host executes later (Makefile, make/, tools/, ...) bound read-only on top and
the maintainers' notes masked, and its own storage under ${XDG_CACHE_HOME:-~/.cache}/oxedi-sandbox:
target/ and .venv/ per checkout, the cargo registry shared. No host variable and no part of the
host home other than that storage reaches it. Two phases, each in its own containers:
- fetch: with network, the checkout read-only, no project code run: `cargo fetch --locked` and
  wheels only for the venv, written to the shared caches; it never falls back to them offline;
- run: the command, with no network and the cargo registry read-only.

usage: sandbox.py --image TAG --checkout DIR
The command comes from SANDBOX_CMD. SANDBOX_CPUS (default: the daemon's CPUs minus one),
SANDBOX_MEMORY (GB; default 60 % of the daemon's memory) and SANDBOX_TIMEOUT (seconds; default
3600) set the limits.
Exit status: the command's; 124 when it ran past the timeout; 130 on Ctrl-C; 2 when the sandbox
refuses to start.
"""

import argparse
import os
import sys
import time
from dataclasses import dataclass
from pathlib import Path

from arguments import Limits, Phase, Runtime, docker_args, fetch_commands, run_command
from layout import Checkout, SandboxError, survey
from lifecycle import daemon_info, explain, image_labels, limits_from, refuse_if_running, run_container
from storage import Storage, cache_root, empty_dir, prepare, storage_for

FETCH_BUDGET_S = 600



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


def fetch_phase(plan: Plan) -> int:
    """Runs the fetch commands, each in its own container, within one budget; stops at the first
    failure."""
    started = time.monotonic()
    # The run phase writes the venv: a planted interpreter or .pth must never run with the network.
    empty_dir(plan.storage.venv)
    for command in fetch_commands():
        remaining = FETCH_BUDGET_S - (time.monotonic() - started)
        outcome = run_container(plan.argv(Phase.FETCH, command), plan.checkout.name, max(1.0, remaining))
        if outcome.status != 0:
            budget = f"the fetch ran past its {FETCH_BUDGET_S} s budget"
            failure = (f"fetch failed: {' '.join(command)} exited {outcome.status}; the fetch needs the network "
                       "and never falls back to the caches offline")
            print(f"sandbox: {explain(outcome, plan.limits, budget, failure)}", file=sys.stderr)
            return outcome.status
    print(f"sandbox: fetch took {time.monotonic() - started:.1f} s", file=sys.stderr)
    return 0


def run_phase(plan: Plan, command: str) -> int:
    started = time.monotonic()
    outcome = run_container(plan.argv(Phase.RUN, run_command(command)), plan.checkout.name, plan.limits.timeout_s)
    print(f"sandbox: run took {time.monotonic() - started:.1f} s", file=sys.stderr)
    if outcome.status != 0:
        budget = f"the command ran past SANDBOX_TIMEOUT={plan.limits.timeout_s} s"
        print(f"sandbox: {explain(outcome, plan.limits, budget, f'command exited {outcome.status}')}", file=sys.stderr)
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
    status = fetch_phase(plan)
    return status if status != 0 else run_phase(plan, command)


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
