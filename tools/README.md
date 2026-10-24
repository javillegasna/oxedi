# Tools

Repository tools used both locally and in CI. A script that belongs to one crate stays in that
crate, and a script only CI runs stays in `.github/scripts/`.

## complexity

The complexity ratchet. `complexity.sh` measures the cyclomatic complexity and nesting of every
Rust, Python and JS function with a pinned lizard and compares them with
`.github/complexity-baseline.txt` through `complexity.py`; `make complexity` and
`make complexity-update` run it. `complexity-baseline-diff.sh` reports, on a pull request, the
baseline entries the change adds or raises. `tests/complexity.sh` checks the comparison on
hand-written measurements. CONTRIBUTING.md (Complexity) explains the targets and the baseline.
