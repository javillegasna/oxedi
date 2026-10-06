# Developer entry points. Every target runs from the repository root.
# Rust gates mirror CI; Python targets use the local .venv (create it with `make venv`).

PY_MANIFEST := crates/oxedi_py/Cargo.toml
PY_TESTS    := crates/oxedi_py/tests
# stubtest reads an allowlist only when one exists; each entry carries its reason.
STUBTEST_ALLOWLIST := crates/oxedi_py/stubtest-allowlist.txt
VENV        := .venv
PYTHON      := $(VENV)/bin/python
# Prefer the venv's tools; fall back to whatever is on PATH (asdf shims, uv tool installs).
MATURIN     := $(if $(wildcard $(VENV)/bin/maturin),$(VENV)/bin/maturin,maturin)
PYTEST      := $(if $(wildcard $(VENV)/bin/pytest),$(VENV)/bin/pytest,$(PYTHON) -m pytest)
WHEELS      := target/wheels
# The workspace version is Cargo's form (0.1.0-rc.1); maturin publishes it as PEP 440 (0.1.0rc1).
# Only the -a.N, -b.N, -rc.N and -dev.N pre-release forms are mapped; any other suffix
# (-alpha, -beta) leaves a version that release-check rejects against the tag.
CARGO_VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
# The DuckDB extension has its own version, in its crate's Cargo.toml.
DUCKDB_VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' crates/oxedi_duckdb/Cargo.toml | head -n 1)
VERSION     := $(shell echo '$(CARGO_VERSION)' | sed -E 's/-(a|b|rc)\.?/\1/; s/-dev\.?/.dev/')

.PHONY: help version release-check configure_ci set_duckdb_version set_duckdb_tag set_duckdb_repository debug release test_debug test_release duckdb-oracle duckdb-version-check duckdb-release-check duckdb-tag sdist-check wheel-check gates test clippy fmt fmt-check bench-check doc venv py-dev py-test stubs stubtest compat-oracle dist smoke publish-test publish-test-verify publish tag clean-dist

help: ## list targets
	@grep -E '^[a-z][a-z_-]*:.*##' $(MAKEFILE_LIST) | sed 's/:.*## /\t/'
	@echo
	@echo "Python releases use release-check and tag (tags v*, which start the PyPI workflow)."
	@echo "The DuckDB extension uses duckdb-release-check and duckdb-tag (tags duckdb-v*, which do not)."

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
	uv pip install --python $(PYTHON) maturin pytest polars pyarrow pandas "edi-835-parser==1.8.0" duckdb mypy pandas-stubs

py-dev: ## build the extension into .venv (debug)
	$(MATURIN) develop --uv --manifest-path $(PY_MANIFEST)

py-test: py-dev ## build and run the Python suite
	$(PYTEST) -q $(PY_TESTS)

stubs: ## regenerate the native module's type stub from the binding
	cargo run --locked -p oxedi_py --bin stub_gen

stubtest: py-dev ## check the stub against the built module, the public API with mypy --strict, and the package with mypy
	$(PYTHON) -m mypy.stubtest oxedi._core $(if $(wildcard $(STUBTEST_ALLOWLIST)),--allowlist $(STUBTEST_ALLOWLIST))
	$(PYTHON) -m mypy --strict $(PY_TESTS)/typing/usage.py
	$(PYTHON) -m mypy --config-file crates/oxedi_py/pyproject.toml crates/oxedi_py/python/oxedi

compat-oracle: ## compare with edi-835-parser on DIR (outside the repo); prints counts and verdicts only
	@test -n "$(DIR)" || (echo "usage: make compat-oracle DIR=/path/outside/the/repo [OUT=report.txt]" && exit 1)
	$(PYTHON) scripts/compat_oracle.py "$(DIR)" $(if $(OUT),--out "$(OUT)")

dist: clean-dist ## build the sdist and the release wheel into target/wheels
	$(MATURIN) sdist --manifest-path $(PY_MANIFEST) -o $(WHEELS)
	@$(MAKE) --no-print-directory sdist-check
	$(MATURIN) build --release --manifest-path $(PY_MANIFEST) -o $(WHEELS)
	@$(MAKE) --no-print-directory wheel-check
	@ls -l $(WHEELS)

# Paths are matched exactly against the archive's listing, under its single top directory.
SDIST_REQUIRED := crates/oxedi_core/src/lib.rs crates/oxedi_core/specs/835.json crates/oxedi_core/specs/835.4010.json crates/oxedi_core/specs/edi_835_parser.json crates/oxedi_core/Cargo.toml crates/oxedi_py/src/lib.rs pyproject.toml LICENSE THIRD_PARTY_NOTICES

sdist-check: ## fail if the sdist holds the core's test trees or lacks what the build needs
	@sdist=$$(ls $(WHEELS)/oxedi-*.tar.gz 2>/dev/null | head -n 1); \
	  test -n "$$sdist" || { echo "sdist-check: no oxedi-*.tar.gz in $(WHEELS); run make dist first"; exit 1; }; \
	  listing=$$(tar tzf "$$sdist" | sed 's|^[^/]*/||') || exit 1; \
	  if echo "$$listing" | grep -E '(^|/)oxedi_core/tests/'; then echo "sdist-check: $$sdist holds the core's tests/ tree (samples, golden, fixtures)"; exit 1; fi; \
	  if echo "$$listing" | grep -E '(^|/)__pycache__(/|$$)|\.pyc$$'; then echo "sdist-check: $$sdist holds compiled bytecode"; exit 1; fi; \
	  for need in $(SDIST_REQUIRED); do \
	    echo "$$listing" | grep -qxF "$$need" || { echo "sdist-check: $$sdist lacks $$need"; exit 1; }; \
	  done; \
	  top=$$(tar tzf "$$sdist" | head -n 1 | cut -d/ -f1); \
	  for f in LICENSE THIRD_PARTY_NOTICES; do \
	    tar xzOf "$$sdist" "$$top/$$f" | cmp -s - "$$f" || { echo "sdist-check: $$f in $$sdist differs from the repository's $$f"; exit 1; }; \
	  done; echo "sdist-check: ok ($$(echo "$$listing" | wc -l) entries)"

wheel-check: ## fail if the built wheel lacks the type stub, py.typed or the licenses, or holds bytecode
	@wheel=$$(ls $(WHEELS)/oxedi-*.whl 2>/dev/null | head -n 1); \
	  test -n "$$wheel" || { echo "wheel-check: no oxedi-*.whl in $(WHEELS); run make dist first"; exit 1; }; \
	  $(PYTHON) scripts/check_wheel.py "$$wheel" .

smoke: ## install the built wheel in a clean venv outside the repo and run the suite
	scripts/smoke_wheel.sh

clean-dist:
	rm -rf $(WHEELS)

# ---- DuckDB extension (crates/oxedi_duckdb) ----
# DuckDB's community CI runs set_duckdb_version, configure_ci, release (or debug) and
# test_release (or test_debug) from the repository root and uploads
# build/<type>/extension/oxedi/oxedi.duckdb_extension, so each target delegates to the crate's
# extension-ci-tools Makefile and the build copies its artifact to that path here.
# These names are fixed by that CI: release, debug, test_release and test_debug build and test the
# DuckDB extension only; Python releases go through dist, publish and tag.
EXT_DIR     := crates/oxedi_duckdb
EXT_NAME    := oxedi
PYTHON_BIN  ?= python3

configure_ci: ## DuckDB extension: create the test venv and record platform and version
	$(MAKE) -C $(EXT_DIR) configure_ci

set_duckdb_version: ## DuckDB extension: no-op for C API extensions (called by DuckDB's CI)
	$(MAKE) -C $(EXT_DIR) set_duckdb_version

set_duckdb_tag: ## DuckDB extension: no-op for C API extensions (called by DuckDB's CI)
	$(MAKE) -C $(EXT_DIR) set_duckdb_tag

set_duckdb_repository: ## DuckDB extension: no-op for C API extensions (called by DuckDB's CI)
	$(MAKE) -C $(EXT_DIR) set_duckdb_repository

release: ## DuckDB extension (not a Python release; see dist/publish/tag): build oxedi.duckdb_extension (release) into build/release/extension/oxedi
	$(MAKE) -C $(EXT_DIR) release
	@$(PYTHON_BIN) -c "import pathlib, shutil; d = pathlib.Path('build/release/extension/$(EXT_NAME)'); d.mkdir(parents=True, exist_ok=True); shutil.copyfile('$(EXT_DIR)/build/release/extension/$(EXT_NAME)/$(EXT_NAME).duckdb_extension', d / '$(EXT_NAME).duckdb_extension')"

debug: ## DuckDB extension: build oxedi.duckdb_extension (debug) into build/debug/extension/oxedi
	$(MAKE) -C $(EXT_DIR) debug
	@$(PYTHON_BIN) -c "import pathlib, shutil; d = pathlib.Path('build/debug/extension/$(EXT_NAME)'); d.mkdir(parents=True, exist_ok=True); shutil.copyfile('$(EXT_DIR)/build/debug/extension/$(EXT_NAME)/$(EXT_NAME).duckdb_extension', d / '$(EXT_NAME).duckdb_extension')"

test_release: ## DuckDB extension: run its SQLLogicTests against the release build
	$(MAKE) -C $(EXT_DIR) test_release

test_debug: ## DuckDB extension: run its SQLLogicTests against the debug build
	$(MAKE) -C $(EXT_DIR) test_debug

duckdb-version-check: ## fail unless the community descriptor's version equals the extension crate's version
	@d=$$(sed -n 's/^  version: *\(.*\)$$/\1/p' $(EXT_DIR)/description.yml | head -n 1); \
	test "$$d" = "$(DUCKDB_VERSION)" || { echo "duckdb-version-check: $(EXT_DIR)/description.yml has version $$d but $(EXT_DIR)/Cargo.toml has $(DUCKDB_VERSION)"; exit 1; }

duckdb-oracle: py-dev ## DuckDB extension: compare read_835 with oxedi.parse_file on every sample and fixture
	$(MAKE) -C $(EXT_DIR) test_oracle ORACLE_PYTHON=$(abspath $(PYTHON))

# ---- Release ----
# Python releases use release-check and tag (tags v*, which start the PyPI workflow).
# The DuckDB extension uses duckdb-release-check and duckdb-tag (tags duckdb-v*, which do not).
version: ## print the PEP 440 version published by maturin
	@echo $(VERSION)

release-check: ## fail unless TAG is v<version>, the tree is clean and CHANGELOG.md has the version
	@test -n "$(TAG)" || { echo "usage: make release-check TAG=v<version>"; exit 1; }
	@test -n "$(VERSION)" || { echo "release-check: no version in Cargo.toml [workspace.package]"; exit 1; }
	@test "$(TAG)" = "v$(VERSION)" || { echo "release-check: tag $(TAG) differs from v$(VERSION) (Cargo.toml version $(CARGO_VERSION))"; exit 1; }
	@test -z "$$(git status --porcelain)" || { echo "release-check: the working tree is not clean"; git status --short; exit 1; }
	@grep -qE '^## \[$(subst .,\.,$(VERSION))\]' CHANGELOG.md || { echo "release-check: CHANGELOG.md has no '## [$(VERSION)]' section"; exit 1; }
	@echo "release-check: ok ($(TAG))"

duckdb-release-check: ## fail unless TAG is duckdb-v<extension version>, the tree is clean, HEAD is on origin/master, the descriptor matches and the extension CHANGELOG.md has the version
	@test -n "$(TAG)" || { echo "usage: make duckdb-release-check TAG=duckdb-v<version>"; exit 1; }
	@test -n "$(DUCKDB_VERSION)" || { echo "duckdb-release-check: no version in $(EXT_DIR)/Cargo.toml"; exit 1; }
	@test "$(TAG)" = "duckdb-v$(DUCKDB_VERSION)" || { echo "duckdb-release-check: tag $(TAG) differs from duckdb-v$(DUCKDB_VERSION) ($(EXT_DIR)/Cargo.toml version $(DUCKDB_VERSION))"; exit 1; }
	@test -z "$$(git status --porcelain)" || { echo "duckdb-release-check: the working tree is not clean"; git status --short; exit 1; }
	@git merge-base --is-ancestor HEAD origin/master || { echo "duckdb-release-check: HEAD is not on origin/master (run git fetch first; the check uses the local origin/master)"; exit 1; }
	@$(MAKE) --no-print-directory duckdb-version-check
	@grep -qE '^## \[$(subst .,\.,$(DUCKDB_VERSION))\]' $(EXT_DIR)/CHANGELOG.md || { echo "duckdb-release-check: $(EXT_DIR)/CHANGELOG.md has no '## [$(DUCKDB_VERSION)]' section"; exit 1; }
	@echo "duckdb-release-check: ok ($(TAG))"

# ---- Publishing (credentials come from ~/.pypirc; never from the repo) ----
publish-test: sdist-check ## upload the built artifacts to TestPyPI
	$(MATURIN) upload -r testpypi $(WHEELS)/oxedi-$(VERSION)*

publish-test-verify: ## install the TestPyPI pre-release into .venv and import it
	uv pip install --python $(PYTHON) --index-url https://test.pypi.org/simple/ --pre --no-deps --reinstall oxedi==$(VERSION)
	$(PYTHON) -c "import oxedi, importlib.metadata as m; print(m.version('oxedi'), oxedi.Spec.builtin())"

publish: sdist-check ## upload the built artifacts to PyPI (irreversible)
	@test -n "$(VERSION)" || (echo "no version in Cargo.toml" && exit 1)
	@echo "about to publish oxedi $(VERSION) to PyPI"; read -p "type the version to confirm: " v && test "$$v" = "$(VERSION)"
	$(MATURIN) upload -r pypi $(WHEELS)/oxedi-$(VERSION)*

tag: ## tag the current commit as v<version> and push the tag
	git tag -a v$(VERSION) -m "oxedi $(VERSION)"
	git push origin v$(VERSION)

duckdb-tag: ## tag the current commit as duckdb-v<extension version> and push the tag (does not start the PyPI workflow)
	git tag -a duckdb-v$(DUCKDB_VERSION) -m "oxedi DuckDB extension $(DUCKDB_VERSION)"
	git push origin duckdb-v$(DUCKDB_VERSION)
