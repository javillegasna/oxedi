# Developer entry points. Every target runs from the repository root.
# Rust gates mirror CI; Python targets use the local .venv (create it with `make venv`).

PY_MANIFEST := crates/oxedi835_py/Cargo.toml
PY_TESTS    := crates/oxedi835_py/tests
VENV        := .venv
PYTHON      := $(VENV)/bin/python
# Prefer the venv's tools; fall back to whatever is on PATH (asdf shims, uv tool installs).
MATURIN     := $(if $(wildcard $(VENV)/bin/maturin),$(VENV)/bin/maturin,maturin)
PYTEST      := $(if $(wildcard $(VENV)/bin/pytest),$(VENV)/bin/pytest,$(PYTHON) -m pytest)
WHEELS      := target/wheels
VERSION     := $(shell sed -n 's/^version = "\(.*\)"/\1/p' crates/oxedi835_py/pyproject.toml)

.PHONY: help gates test clippy fmt fmt-check bench-check doc venv py-dev py-test compat-oracle dist smoke publish-test publish-test-verify publish tag clean-dist

help: ## list targets
	@grep -E '^[a-z][a-z-]*:.*##' $(MAKEFILE_LIST) | sed 's/:.*## /\t/'

# ---- Rust gates (the same four CI runs, plus rustdoc) ----
gates: fmt-check clippy test bench-check doc ## run every commit gate

test: ## cargo test --workspace --locked
	cargo test --workspace --locked

clippy: ## clippy with warnings as errors
	cargo clippy --workspace --all-targets --locked -- -D warnings

fmt: ## format the workspace
	cargo fmt --all

fmt-check: ## check formatting
	cargo fmt --all -- --check

bench-check: ## compile the benches without running them
	cargo bench --workspace --no-run --locked

doc: ## rustdoc with warnings as errors
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked

# ---- Python binding ----
venv: ## create .venv with uv and the dev tools
	uv venv $(VENV) --python 3.13
	uv pip install --python $(PYTHON) maturin pytest polars pyarrow pandas "edi-835-parser==1.8.0" duckdb

py-dev: ## build the extension into .venv (debug)
	$(MATURIN) develop --uv --manifest-path $(PY_MANIFEST)

py-test: py-dev ## build and run the Python suite
	$(PYTEST) -q $(PY_TESTS)

compat-oracle: ## compare with edi-835-parser on DIR (outside the repo); prints counts and verdicts only
	@test -n "$(DIR)" || (echo "usage: make compat-oracle DIR=/path/outside/the/repo [OUT=report.txt]" && exit 1)
	$(PYTHON) scripts/compat_oracle.py "$(DIR)" $(if $(OUT),--out "$(OUT)")

dist: clean-dist ## build the sdist and the release wheel into target/wheels
	$(MATURIN) sdist --manifest-path $(PY_MANIFEST) -o $(WHEELS)
	$(MATURIN) build --release --manifest-path $(PY_MANIFEST) -o $(WHEELS)
	@ls -l $(WHEELS)

smoke: ## install the built wheel in a clean venv outside the repo and run the suite
	scripts/smoke_wheel.sh

clean-dist:
	rm -rf $(WHEELS)

# ---- Publishing (credentials come from ~/.pypirc; never from the repo) ----
publish-test: ## upload the built artifacts to TestPyPI
	$(MATURIN) upload -r testpypi $(WHEELS)/oxedi835-$(VERSION)*

publish-test-verify: ## install the TestPyPI pre-release into .venv and import it
	uv pip install --python $(PYTHON) --index-url https://test.pypi.org/simple/ --pre --no-deps --reinstall oxedi835==$(VERSION)
	$(PYTHON) -c "import oxedi835, importlib.metadata as m; print(m.version('oxedi835'), oxedi835.Spec.builtin())"

publish: ## upload the built artifacts to PyPI (irreversible)
	@test -n "$(VERSION)" || (echo "no version in pyproject.toml" && exit 1)
	@echo "about to publish oxedi835 $(VERSION) to PyPI"; read -p "type the version to confirm: " v && test "$$v" = "$(VERSION)"
	$(MATURIN) upload -r pypi $(WHEELS)/oxedi835-$(VERSION)*

tag: ## tag the current commit as v<version> and push the tag
	git tag -a v$(VERSION) -m "oxedi835 $(VERSION)"
	git push origin v$(VERSION)
