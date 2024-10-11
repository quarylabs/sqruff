.PHONY: help
help: ## Display this help screen
	@grep -E '^[a-z.A-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

.PHONY: python_fmt
python_fmt: ## Format python code
	ruff format .

.PHONY: python_lint
python_lint: ## Lint python code
	ruff check .

.PHONY: python_test
python_test: ## Run python tests
	pytest

.PHONY: ci
ci: python_fmt python_lint python_test ## Run all CI checks