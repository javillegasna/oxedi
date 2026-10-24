# Tools

Repository tools used both locally and in CI. A script that belongs to one crate stays in that
crate, and a script only CI runs stays in `.github/scripts/`.

## deps

The check that no development dependency reaches what oxedi publishes. `deps.py crates` compares
the normal dependencies `cargo tree` reports for `oxedi_core`, `oxedi_py`, `oxedi_duckdb` and
`oxedi_wasm` with the allowed list kept in the script; `deps.py wheel` requires every
`Requires-Dist` of a built wheel to carry an `extra ==` marker of a user extra. `make deps-check`
runs the crate check and, when `make dist` left a wheel in `target/wheels`, the wheel check;
`make gates` includes it. `tests/deps.sh` proves both fail on a leak, with a fake `cargo` and
hand-built wheels.

## complexity

The complexity ratchet. `complexity.sh` measures the cyclomatic complexity and nesting of every
Rust, Python and JS function with a pinned lizard and compares them with
`.github/complexity-baseline.txt` through `complexity.py`; `make complexity` and
`make complexity-update` run it. `complexity-baseline-diff.sh` reports, on a pull request, the
baseline entries the change adds or raises. `tests/complexity.sh` checks the comparison on
hand-written measurements. CONTRIBUTING.md (Complexity) explains the targets and the baseline.

## sandbox

The container `make sandbox` builds and tests in. `Dockerfile` is the pinned image. `sandbox.py`
is the runner; its parts are `layout.py` (which paths are read-only, masked or hidden),
`storage.py` (the storage on the host and `make sandbox-clean`), `arguments.py` (the `docker run`
arguments of each phase), `lifecycle.py` (the daemon, the limits, one container's run and kill)
and `audit.py` (what a run changed or left in the checkout). `isolation-check.sh` runs inside the
container for `make sandbox-check`. `tests/sandbox.sh` runs the runner's tests against a fake
`docker` (`tests/fake_docker.py`), so they need no Docker. CONTRIBUTING.md (Sandbox) explains
how to use it.
