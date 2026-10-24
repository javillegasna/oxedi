# The sandbox image: the pinned toolchain that builds and tests the repository inside a container.
# Included by the root Makefile.
.PHONY: sandbox-image

##@ Sandbox
SANDBOX_DIR   := tools/sandbox
# The tag is the first 12 hex digits of the Dockerfile's SHA-256, so a changed Dockerfile is a new tag.
SANDBOX_HASH   = $(shell $(PYTHON_BIN) -c "import hashlib, pathlib; print(hashlib.sha256(pathlib.Path('$(SANDBOX_DIR)/Dockerfile').read_bytes()).hexdigest()[:12])")
SANDBOX_IMAGE  = oxedi-sandbox:$(SANDBOX_HASH)

# Two builds: the second reuses every layer and only adds the apt set, read from the first, as a label.
sandbox-image: ## build the sandbox image from tools/sandbox (tag oxedi-sandbox:<hash of the Dockerfile>)
	docker build --tag $(SANDBOX_IMAGE) $(SANDBOX_DIR)
	apt=$$(docker run --rm --pull never --network none --read-only --cap-drop ALL --user 65534:65534 --security-opt no-new-privileges $(SANDBOX_IMAGE) cat /usr/share/oxedi-sandbox.manifest | tr '\t\n' '= ' | sed 's/ $$//') && \
	  docker build --tag $(SANDBOX_IMAGE) --label "oxedi.sandbox.apt=$$apt" $(SANDBOX_DIR)
	@echo "sandbox-image: $(SANDBOX_IMAGE), $$(docker image inspect --format '{{.Size}}' $(SANDBOX_IMAGE) | awk '{ printf "%.0f MB", $$1 / 1000000 }')"
