#!/usr/bin/env bash
# Builds the release wheel, installs it in a fresh virtual environment
# outside the repository and runs the Python tests there against it.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

maturin build --release --manifest-path "$repo/crates/oxedi835_py/Cargo.toml" --out "$work/dist"
uv venv --quiet --python "$(command -v python)" "$work/venv"
VIRTUAL_ENV="$work/venv" uv pip install --quiet "$work"/dist/oxedi835-*.whl pytest polars pyarrow
cp -r "$repo/crates/oxedi835_py/tests" "$work/tests"

cd "$work"
"$work/venv/bin/python" -c "
import oxedi835, pathlib
assert pathlib.Path(oxedi835.__file__).is_relative_to(pathlib.Path('$work/venv')), oxedi835.__file__
result = oxedi835.parse_file('$repo/crates/edi835_core/tests/samples/edi835_test_united.rmt')
print('smoke:', result)
"
OXEDI835_CORE_TESTS="$repo/crates/edi835_core/tests" "$work/venv/bin/python" -m pytest -q -p no:cacheprovider "$work/tests"
