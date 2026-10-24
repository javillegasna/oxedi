# The sandbox: a pinned image and a runner that builds and tests this checkout inside a hardened,
# throwaway container (tools/sandbox/sandbox.py says how).
# Included by the root Makefile.
.PHONY: sandbox-image sandbox sandbox-check sandbox-clean

##@ Sandbox
SANDBOX_DIR   := tools/sandbox
# Every Python this file starts runs isolated (-I): a module a run left in the checkout, such as a
# root hashlib.py, or a PYTHON* variable of the shell is never loaded.
SANDBOX_PY    := $(PYTHON_BIN) -I
# The tag is the first 12 hex digits of the Dockerfile's SHA-256, so a changed Dockerfile is a new tag.
SANDBOX_HASH   = $(shell $(SANDBOX_PY) -c "import hashlib, pathlib; print(hashlib.sha256(pathlib.Path('$(SANDBOX_DIR)/Dockerfile').read_bytes()).hexdigest()[:12])")
SANDBOX_IMAGE  = oxedi-sandbox:$(SANDBOX_HASH)

# Two builds: the second reuses every layer and only adds the apt set, read from the first, as a label.
sandbox-image: ## build the sandbox image from tools/sandbox (tag oxedi-sandbox:<hash of the Dockerfile>)
	docker build --tag $(SANDBOX_IMAGE) $(SANDBOX_DIR)
	apt=$$(docker run --rm --pull never --network none --read-only --cap-drop ALL --user 65534:65534 --security-opt no-new-privileges $(SANDBOX_IMAGE) cat /usr/share/oxedi-sandbox.manifest | tr '\t\n' '= ' | sed 's/ $$//') && \
	  docker build --tag $(SANDBOX_IMAGE) --label "oxedi.sandbox.apt=$$apt" $(SANDBOX_DIR)
	@echo "sandbox-image: $(SANDBOX_IMAGE), $$(docker image inspect --format '{{.Size}}' $(SANDBOX_IMAGE) | awk '{ printf "%.0f MB", $$1 / 1000000 }')"

# CMD reaches the container's `bash -c` as SANDBOX_CMD, expanded once by make ($$ is a literal $)
# and with no other quoting layer. A missing image is built first, through sandbox-image.
sandbox: export SANDBOX_CMD := $(CMD)
sandbox: ## run CMD over this checkout in a hardened container with no network (make sandbox CMD="make gates")
	@if command -v docker > /dev/null && ! docker image inspect $(SANDBOX_IMAGE) > /dev/null 2>&1; then \
	  $(MAKE) --no-print-directory sandbox-image; \
	fi
	@$(SANDBOX_PY) $(SANDBOX_DIR)/sandbox.py --image $(SANDBOX_IMAGE) --checkout $(CURDIR)

# The host home path is an argument, so the check proves that path is not mounted.
sandbox-check: ## prove the sandbox's isolation from inside it: every line of tools/sandbox/isolation-check.sh says ok
	@$(MAKE) --no-print-directory sandbox CMD='bash $(SANDBOX_DIR)/isolation-check.sh "$(HOME)"'

sandbox-clean: ## remove this checkout's sandbox storage (ALL=1: every checkout's and the shared caches)
	@$(SANDBOX_PY) $(SANDBOX_DIR)/sandbox.py --clean $(if $(ALL),all,checkout) --checkout $(CURDIR)
