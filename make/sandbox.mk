# The sandbox: a pinned image and a runner that builds and tests this checkout inside a hardened,
# throwaway container (tools/sandbox/sandbox.py says how).
# Included by the root Makefile.
.PHONY: sandbox-image sandbox-image-viewer sandbox sandbox-pr sandbox-check sandbox-pr-check sandbox-tools-check sandbox-clean

##@ Sandbox
SANDBOX_DIR   := tools/sandbox
# Every Python this file starts runs isolated (-I): a module a run left in the checkout, such as a
# root hashlib.py, or a PYTHON* variable of the shell is never loaded.
SANDBOX_PY    := $(PYTHON_BIN) -I
# The tag is the first 12 hex digits of the SHA-256 of the Dockerfile followed by pip-audit.txt, the
# two files the build reads (the Dockerfile COPYs pip-audit.txt), so a change to either is a new tag.
SANDBOX_HASH   = $(shell $(SANDBOX_PY) -c "import hashlib, pathlib; print(hashlib.sha256(b''.join(pathlib.Path('$(SANDBOX_DIR)/' + n).read_bytes() for n in ('Dockerfile', 'pip-audit.txt'))).hexdigest()[:12])")
SANDBOX_IMAGE  = oxedi-sandbox:$(SANDBOX_HASH)
# The viewer image is built on the base one: its tag follows the base tag, its Dockerfile, playwright.txt,
# install_tools.sh and viewer_image.py (that file derives the tag and the wasm and Playwright pins).
SANDBOX_VIEWER_IMAGE = $(shell $(SANDBOX_PY) $(SANDBOX_DIR)/viewer_image.py tag --root $(CURDIR) --base $(SANDBOX_IMAGE))
SANDBOX_DOCKER_ARCH  = $(shell docker version --format '{{.Server.Arch}}' 2> /dev/null)

# Two builds: the second reuses every layer and only adds the apt set, read from the first, as a label.
sandbox-image: ## build the sandbox image from tools/sandbox (tag oxedi-sandbox:<hash of the Dockerfile and pip-audit.txt>)
	docker build --tag $(SANDBOX_IMAGE) $(SANDBOX_DIR)
	apt=$$(docker run --rm --pull never --network none --read-only --cap-drop ALL --user 65534:65534 --security-opt no-new-privileges $(SANDBOX_IMAGE) cat /usr/share/oxedi-sandbox.manifest | tr '\t\n' '= ' | sed 's/ $$//') && \
	  docker build --tag $(SANDBOX_IMAGE) --label "oxedi.sandbox.apt=$$apt" $(SANDBOX_DIR)
	@echo "sandbox-image: $(SANDBOX_IMAGE), $$(docker image inspect --format '{{.Size}}' $(SANDBOX_IMAGE) | awk '{ printf "%.0f MB", $$1 / 1000000 }')"

# Two builds, like sandbox-image. The base image is built first when missing; the wasm tools are
# pinned for x86_64, so another docker server architecture is refused.
sandbox-image-viewer: ## build the viewer sandbox image (Node, Chromium, wasm tools) on the base image (amd64 only)
	@test "$(SANDBOX_DOCKER_ARCH)" = amd64 || { echo "sandbox-image-viewer: the docker server is '$(SANDBOX_DOCKER_ARCH)' and the wasm tools are pinned for amd64" >&2; exit 1; }
	@test -n "$(SANDBOX_VIEWER_IMAGE)" || exit 1
	@docker image inspect $(SANDBOX_IMAGE) > /dev/null 2>&1 || $(MAKE) --no-print-directory sandbox-image
	args=$$($(SANDBOX_PY) $(SANDBOX_DIR)/viewer_image.py build-args --root $(CURDIR)) && \
	  docker build -f $(SANDBOX_DIR)/Dockerfile.viewer --build-arg BASE_IMAGE=$(SANDBOX_IMAGE) $$args --tag $(SANDBOX_VIEWER_IMAGE) $(SANDBOX_DIR) && \
	  apt=$$(docker run --rm --pull never --network none --read-only --cap-drop ALL --user 65534:65534 --security-opt no-new-privileges $(SANDBOX_VIEWER_IMAGE) cat /usr/share/oxedi-sandbox.manifest | tr '\t\n' '= ' | sed 's/ $$//') && \
	  docker build -f $(SANDBOX_DIR)/Dockerfile.viewer --build-arg BASE_IMAGE=$(SANDBOX_IMAGE) $$args --tag $(SANDBOX_VIEWER_IMAGE) --label "oxedi.sandbox.apt=$$apt" $(SANDBOX_DIR)
	@echo "sandbox-image-viewer: $(SANDBOX_VIEWER_IMAGE), $$(docker image inspect --format '{{.Size}}' $(SANDBOX_VIEWER_IMAGE) | awk '{ printf "%.0f MB", $$1 / 1000000 }')"

# CMD reaches the container's `bash -c` as SANDBOX_CMD, expanded once by make ($$ is a literal $)
# and with no other quoting layer. A missing image is built first, through sandbox-image.
sandbox: export SANDBOX_CMD := $(CMD)
sandbox: ## run CMD over this checkout in a hardened container with no network (make sandbox CMD="make gates")
	@if command -v docker > /dev/null && ! docker image inspect $(SANDBOX_IMAGE) > /dev/null 2>&1; then \
	  $(MAKE) --no-print-directory sandbox-image; \
	fi
	@$(SANDBOX_PY) $(SANDBOX_DIR)/sandbox.py --image $(SANDBOX_IMAGE) --checkout $(CURDIR)

# The change, the target and the confirmation reach sandbox_pr.py as SANDBOX_* variables, like CMD
# for sandbox. The rules (sandbox_pr.py, risk.py, deny.toml) are this checkout's, never the change's.
sandbox-pr: export SANDBOX_PR := $(PR)
sandbox-pr: export SANDBOX_REF := $(REF)
sandbox-pr: export SANDBOX_HEAD := $(HEAD)
sandbox-pr: export SANDBOX_TARGET := $(TARGET)
sandbox-pr: export SANDBOX_CONFIRM := $(CONFIRM)
sandbox-pr: ## test a pull request (PR=N [HEAD=1]) or a ref (REF=<name>) in the sandbox, after a risk report [TARGET="..."] [CONFIRM=yes]
	@if command -v docker > /dev/null && ! docker image inspect $(SANDBOX_IMAGE) > /dev/null 2>&1; then \
	  $(MAKE) --no-print-directory sandbox-image; \
	fi
	@$(SANDBOX_PY) $(SANDBOX_DIR)/sandbox_pr.py --image $(SANDBOX_IMAGE) --checkout $(CURDIR)

# The host home path is an argument, so the check proves that path is not mounted.
sandbox-check: ## prove the sandbox's isolation from inside it: every line of tools/sandbox/isolation-check.sh says ok
	@$(MAKE) --no-print-directory sandbox CMD='bash $(SANDBOX_DIR)/isolation-check.sh "$(HOME)"'

# The planted change lives only on a local branch of a temporary clone, removed at the end.
sandbox-pr-check: ## prove sandbox-pr keeps a hostile change from the host, the network and the caches: every line of tools/sandbox/malicious-pr.sh says ok
	@if command -v docker > /dev/null && ! docker image inspect $(SANDBOX_IMAGE) > /dev/null 2>&1; then \
	  $(MAKE) --no-print-directory sandbox-image; \
	fi
	@bash $(SANDBOX_DIR)/malicious-pr.sh $(SANDBOX_IMAGE) $(CURDIR)

# Needs the network once: it fetches the advisory database into a temporary directory.
sandbox-tools-check: ## prove cargo-deny and pip-audit work in the image: fetch the advisory database, then check a crate offline
	@if command -v docker > /dev/null && ! docker image inspect $(SANDBOX_IMAGE) > /dev/null 2>&1; then \
	  $(MAKE) --no-print-directory sandbox-image; \
	fi
	@bash $(SANDBOX_DIR)/audit-tools-check.sh $(SANDBOX_IMAGE)

sandbox-clean: ## remove this checkout's sandbox storage (ALL=1: every checkout's and the shared caches)
	@$(SANDBOX_PY) $(SANDBOX_DIR)/sandbox.py --clean $(if $(ALL),all,checkout) --checkout $(CURDIR)
