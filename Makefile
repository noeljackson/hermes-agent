REFERENCE_REPO ?= https://github.com/NousResearch/hermes-agent.git
REFERENCE_REF ?= main
PARITY_FIXTURES_DIR ?= tests/fixtures/python-parity
PARITY_IMAGE ?= hermes-python-parity

.PHONY: check python-parity-build python-parity-fixtures python-parity-update python-parity-drift python-parity-agent real-provider-smoke real-gateway-smoke

check:
	@if [ -f Cargo.toml ]; then cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace; else echo "No Rust workspace yet; skipping cargo test."; fi

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
