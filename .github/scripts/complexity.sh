#!/usr/bin/env bash
# Measures cyclomatic complexity and nesting of every function in the Rust, Python and
# JavaScript sources and compares them with .github/complexity-baseline.txt (complexity.py says
# how). Measured: crates/*/src (Rust), crates/*/python and crates/*/scripts (Python) and
# viewer/js. Not measured: tests (tests/, test/, tests.rs), benches, examples, the tooling under
# .github/, docs/ and viewer/tests, and anything git does not track (target/, site/, viewer/pkg/)
# or that is not a source file (the generated .pyi stubs).
# A baseline function the change modifies must end within the targets. The change is measured
# against $COMPLEXITY_BASE; unset, that is the commit before HEAD in CI on a push or a pull
# request (the checkout is the merge with the base branch, whose first parent is the base) and
# origin/master elsewhere.
# Needs uv: uvx runs complexity.py with the pinned lizard. Arguments go to complexity.py:
# --update rewrites the baseline, --accept PATH::FUNCTION records a reviewed exception.
set -euo pipefail

LIZARD_VERSION="1.24.1"
PYGMENTS_VERSION="2.21.0"
PATHSPEC_VERSION="1.1.1"
PYTHON_VERSION="3.13"

cd "$(dirname "$0")/../.."
case "${GITHUB_EVENT_NAME:-}" in
  push | pull_request) : "${COMPLEXITY_BASE:=HEAD^1}" ;;
esac
export COMPLEXITY_BASE
files=$(mktemp)
trap 'rm -f "$files"' EXIT

git ls-files -- \
  'crates/*/src/*.rs' 'crates/*/python/*.py' 'crates/*/scripts/*.py' 'viewer/js/*.js' \
  | grep -vE '(^|/)tests?(/|\.rs$)' > "$files"

uvx --python "$PYTHON_VERSION" --from "lizard==$LIZARD_VERSION" \
  --with "pygments==$PYGMENTS_VERSION" --with "pathspec==$PATHSPEC_VERSION" \
  python .github/scripts/complexity.py .github/complexity-baseline.txt --files "$files" "$@"
