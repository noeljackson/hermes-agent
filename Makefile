REFERENCE_REPO ?= https://github.com/NousResearch/hermes-agent.git
REFERENCE_REF ?= main
PARITY_FIXTURES_DIR ?= tests/fixtures/python-parity
PARITY_IMAGE ?= hermes-python-parity

.PHONY: check coverage coverage-html coverage-lcov python-parity-build python-parity-fixtures python-parity-update python-parity-drift python-parity-agent real-provider-smoke real-gateway-smoke

check:
	@if [ -f Cargo.toml ]; then cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace; else echo "No Rust workspace yet; skipping cargo test."; fi

coverage:
	@if ! command -v cargo-llvm-cov >/dev/null 2>&1; then \
		echo "cargo-llvm-cov is required for Rust coverage."; \
		echo "Install it with: cargo install cargo-llvm-cov --locked"; \
		exit 127; \
	fi
	cargo llvm-cov --workspace --all-targets --summary-only

coverage-html:
	@if ! command -v cargo-llvm-cov >/dev/null 2>&1; then \
		echo "cargo-llvm-cov is required for Rust coverage."; \
		echo "Install it with: cargo install cargo-llvm-cov --locked"; \
		exit 127; \
	fi
	cargo llvm-cov --workspace --all-targets --html --output-dir target/coverage/html

coverage-lcov:
	@if ! command -v cargo-llvm-cov >/dev/null 2>&1; then \
		echo "cargo-llvm-cov is required for Rust coverage."; \
		echo "Install it with: cargo install cargo-llvm-cov --locked"; \
		exit 127; \
	fi
	mkdir -p target/coverage
	cargo llvm-cov --workspace --all-targets --lcov --output-path target/coverage/lcov.info

python-parity-build:
	docker build -f Dockerfile.python-parity \
		--build-arg REFERENCE_REPO="$(REFERENCE_REPO)" \
		--build-arg REFERENCE_REF="$(REFERENCE_REF)" \
		-t $(PARITY_IMAGE) .

python-parity-fixtures: python-parity-build
	mkdir -p "$(PARITY_FIXTURES_DIR)"
	docker run --rm -v "$(CURDIR)/$(PARITY_FIXTURES_DIR):/fixtures" $(PARITY_IMAGE)

python-parity-update:
	$(MAKE) python-parity-fixtures REFERENCE_REF=main

python-parity-drift:
	scripts/python-parity-drift.sh

python-parity-agent: python-parity-build
	docker run --rm -it \
		-v "$(CURDIR)/$(PARITY_FIXTURES_DIR):/fixtures" \
		--entrypoint /bin/bash \
		$(PARITY_IMAGE)

real-provider-smoke:
	bash scripts/real-provider-smoke.sh

real-gateway-smoke:
	bash scripts/real-gateway-smoke.sh
