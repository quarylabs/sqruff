.PHONY: prettier_fmt
prettier_fmt: ## Formats gh yaml files
	npx prettier --write  .github/**/*.{yaml,yml}

.PHONY: prettier_lint
prettier_lint: ## Lints gh yaml files
	npx prettier --check .github/**/*.{yaml,yml}

.PHONY: help
help: ## Display this help screen
	@grep -E '^[a-z.A-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'