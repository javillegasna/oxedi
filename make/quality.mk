# Complexity ratchet. Included by the root Makefile.
.PHONY: complexity complexity-update

##@ Code quality

# Needs uv; CI runs it in the Workflows job. The targets and the baseline are explained in
# CONTRIBUTING.md (Complexity).
complexity: ## fail if a Rust, Python or JS function is over the complexity targets and not in the baseline, worse than it, or modified while still over a target
	tools/complexity/complexity.sh

complexity-update: ## rewrite .github/complexity-baseline.txt after a refactor or move; ACCEPT='path::function ...' records reviewed exceptions
	tools/complexity/complexity.sh --update $(foreach a,$(ACCEPT),--accept '$(a)')
