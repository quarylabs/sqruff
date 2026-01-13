.PHONY: help
help: ## Display this help screen
	@grep -E '^[a-z.A-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

.PHONY: python_fmt
python_fmt: ## Format python code
	uv run ruff format .

.PHONY: python_lint
python_lint: ## Lint python code
	uv run ruff format --check .
	uv run ruff check .

.PHONY: python_test
python_test: ## Run python tests
	uv run pytest

.PHONY: python_sync
python_sync: ## Sync python dependencies using uv (uses uv.lock)
	uv sync --extra dev

.PHONY: python_generate_gha
python_generate_gha: ## Generate GitHub Actions workflow
	maturin generate-ci github --manifest-path "crates/cli/Cargo.toml" --output .github/workflows/python-ci.yaml

.PHONY: python_ci
python_ci: python_test ## Run python CI (ruff checks moved to bazel) 

.PHONY: rust_fmt
rust_fmt: ## Format rust code
	cargo fmt --all

.PHONY: rust_lint
rust_lint: ## Lint rust code
	cargo fmt --all -- --check
	cargo clippy --all --all-features -- -D warnings
	cargo machete
	cargo hack check --each-feature --exclude-features=codegen-docs

.PHONY: rust_test
rust_test: ## Run rust tests
	cd crates/cli-python && uv run maturin develop
	cargo test --no-fail-fast --manifest-path ./crates/cli/Cargo.toml
	cargo test --no-fail-fast --all --all-features --exclude sqruff

.PHONY: ci
ci: ratchet_check python_ci rust_test ## Run all CI checks

.PHONY: ratchet_pin
ratchet_pin: ## Pins all the Github workflow versions
	ratchet pin .github/workflows/*

.PHONY: ratchet_update
ratchet_update: ## Updates all the Github workflow versions
	ratchet update .github/workflows/*

.PHONY: ratchet_check
ratchet_check: ## Checks all the Github workflow versions
	ratchet lint .github/workflows/*

.PHONY: load_vscode_settings
load_vscode_settings: ## Loads the sample vscode settings
	mkdir -p .vscode
	cp -f .hacking/vscode/* .vscode/
