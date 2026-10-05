# Developer entry points. Every target runs from the repository root.
# Rust gates mirror CI; Python targets use the local .venv (create it with `make venv`).

PY_MANIFEST := crates/oxedi835_py/Cargo.toml
PY_TESTS    := crates/oxedi835_py/tests
# stubtest reads an allowlist only when one exists; each entry carries its reason.
STUBTEST_ALLOWLIST := crates/oxedi835_py/stubtest-allowlist.txt
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
VERSION     := $(shell echo '$(CARGO_VERSION)' | sed -E 's/-(a|b|rc)\.?/\1/; s/-dev\.?/.dev/')

.PHONY: help version release-check sdist-check wheel-check gates test clippy fmt fmt-check bench-check doc venv py-dev py-test stubs stubtest compat-oracle dist smoke publish-test publish-test-verify publish tag clean-dist

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
	uv pip install --python $(PYTHON) maturin pytest polars pyarrow pandas "edi-835-parser==1.8.0" duckdb mypy pandas-stubs

py-dev: ## build the extension into .venv (debug)
	$(MATURIN) develop --uv --manifest-path $(PY_MANIFEST)

py-test: py-dev ## build and run the Python suite
	$(PYTEST) -q $(PY_TESTS)

stubs: ## regenerate the native module's type stub from the binding
	cargo run --locked -p oxedi835_py --bin stub_gen

stubtest: py-dev ## check the stub against the built module and the public API with mypy --strict
	$(PYTHON) -m mypy.stubtest oxedi835._core $(if $(wildcard $(STUBTEST_ALLOWLIST)),--allowlist $(STUBTEST_ALLOWLIST))
	$(PYTHON) -m mypy --strict $(PY_TESTS)/typing/usage.py

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
SDIST_REQUIRED := crates/edi835_core/src/lib.rs crates/edi835_core/specs/835.json crates/edi835_core/specs/835.4010.json crates/edi835_core/specs/edi_835_parser.json crates/edi835_core/Cargo.toml crates/oxedi835_py/src/lib.rs pyproject.toml LICENSE THIRD_PARTY_NOTICES

sdist-check: ## fail if the sdist holds the core's test trees or lacks what the build needs
	@sdist=$$(ls $(WHEELS)/oxedi835-*.tar.gz 2>/dev/null | head -n 1); \
	  test -n "$$sdist" || { echo "sdist-check: no oxedi835-*.tar.gz in $(WHEELS); run make dist first"; exit 1; }; \
	  listing=$$(tar tzf "$$sdist" | sed 's|^[^/]*/||') || exit 1; \
	  if echo "$$listing" | grep -E '(^|/)edi835_core/tests/'; then echo "sdist-check: $$sdist holds the core's tests/ tree (samples, golden, fixtures)"; exit 1; fi; \
	  if echo "$$listing" | grep -E '(^|/)__pycache__(/|$$)|\.pyc$$'; then echo "sdist-check: $$sdist holds compiled bytecode"; exit 1; fi; \
	  for need in $(SDIST_REQUIRED); do \
	    echo "$$listing" | grep -qxF "$$need" || { echo "sdist-check: $$sdist lacks $$need"; exit 1; }; \
	  done; \
	  top=$$(tar tzf "$$sdist" | head -n 1 | cut -d/ -f1); \
	  for f in LICENSE THIRD_PARTY_NOTICES; do \
	    tar xzOf "$$sdist" "$$top/$$f" | cmp -s - "$$f" || { echo "sdist-check: $$f in $$sdist differs from the repository's $$f"; exit 1; }; \
	  done; echo "sdist-check: ok ($$(echo "$$listing" | wc -l) entries)"

wheel-check: ## fail if the built wheel lacks the type stub, py.typed or the licenses, or holds bytecode
	@wheel=$$(ls $(WHEELS)/oxedi835-*.whl 2>/dev/null | head -n 1); \
	  test -n "$$wheel" || { echo "wheel-check: no oxedi835-*.whl in $(WHEELS); run make dist first"; exit 1; }; \
	  $(PYTHON) scripts/check_wheel.py "$$wheel" .

smoke: ## install the built wheel in a clean venv outside the repo and run the suite
	scripts/smoke_wheel.sh

clean-dist:
	rm -rf $(WHEELS)

# ---- Release ----
version: ## print the PEP 440 version published by maturin
	@echo $(VERSION)

release-check: ## fail unless TAG is v<version>, the tree is clean and CHANGELOG.md has the version
	@test -n "$(TAG)" || { echo "usage: make release-check TAG=v<version>"; exit 1; }
	@test -n "$(VERSION)" || { echo "release-check: no version in Cargo.toml [workspace.package]"; exit 1; }
	@test "$(TAG)" = "v$(VERSION)" || { echo "release-check: tag $(TAG) differs from v$(VERSION) (Cargo.toml version $(CARGO_VERSION))"; exit 1; }
	@test -z "$$(git status --porcelain)" || { echo "release-check: the working tree is not clean"; git status --short; exit 1; }
	@grep -qE '^## \[$(subst .,\.,$(VERSION))\]' CHANGELOG.md || { echo "release-check: CHANGELOG.md has no '## [$(VERSION)]' section"; exit 1; }
	@echo "release-check: ok ($(TAG))"

# ---- Publishing (credentials come from ~/.pypirc; never from the repo) ----
publish-test: sdist-check ## upload the built artifacts to TestPyPI
	$(MATURIN) upload -r testpypi $(WHEELS)/oxedi835-$(VERSION)*

publish-test-verify: ## install the TestPyPI pre-release into .venv and import it
	uv pip install --python $(PYTHON) --index-url https://test.pypi.org/simple/ --pre --no-deps --reinstall oxedi835==$(VERSION)
	$(PYTHON) -c "import oxedi835, importlib.metadata as m; print(m.version('oxedi835'), oxedi835.Spec.builtin())"

publish: sdist-check ## upload the built artifacts to PyPI (irreversible)
	@test -n "$(VERSION)" || (echo "no version in Cargo.toml" && exit 1)
	@echo "about to publish oxedi835 $(VERSION) to PyPI"; read -p "type the version to confirm: " v && test "$$v" = "$(VERSION)"
	$(MATURIN) upload -r pypi $(WHEELS)/oxedi835-$(VERSION)*

tag: ## tag the current commit as v<version> and push the tag
	git tag -a v$(VERSION) -m "oxedi835 $(VERSION)"
	git push origin v$(VERSION)
