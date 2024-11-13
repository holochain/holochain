STATIC_CHECKS := static-fmt static-toml static-clippy static-doc

.PHONY: check $(STATIC_CHECKS) format build build-each test

# Run all static checks
check: $(STATIC_CHECKS)

# Check formatting
static-fmt:
	cargo fmt --check

# Check TOML formatting
static-toml:
	taplo format --check ./*.toml
	taplo format --check ./crates/**/*.toml

# Ensure our chosen style lints are followed
static-clippy:
	cargo clippy --all-targets -- --deny warnings

# Check that the docs for this workspace can by generated without warnings
static-doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --no-deps

# Format Rust and TOML files
format:
	cargo fmt
	taplo format ./*.toml
	taplo format ./crates/**/*.toml

# Build the workspace
build:
	cargo build \
		--workspace \
		--locked \
		--all-targets

# Build each crate in the workspace, to find issues with merged features
PROJECTS=$(wildcard crates/*)
build-each:
	@$(foreach PROJ,$(PROJECTS),\
		cd $(PROJ); cargo build --locked; cd ../.. ; \
		)

# Run all the tests
test:
	RUST_BACKTRACE=1 cargo nextest run \
		--workspace \
		--locked \
		--no-fail-fast

bump:
	@if [ "$(ver)x" = "x" ]; then \
		echo "USAGE: make bump ver=0.5.0-dev.1"; \
		exit 1; \
	fi
	sed -i 's/^\(kitsune_p2p[^=]*= { \|\)version = "[^"]*"/\1version = "$(ver)"/g' $$(find . -name Cargo.toml)

RELEASE_ORDER=\
	mdns \
	timestamp \
	dht_arc \
	dht \
	bin_data \
	types \
	fetch \
	bootstrap \
	bootstrap_client \
	kitsune_p2p
publish: SHELL:=/usr/bin/env bash
publish:
	set -euo pipefail
	git diff --exit-code
	@$(foreach CRATE,$(RELEASE_ORDER),\
		cd crates/$(CRATE); \
		NAME="$$(grep name Cargo.toml | head -1 | cut -d ' ' -f 3 | cut -d \" -f 2)"; \
		VER="v$$(grep version Cargo.toml | head -1 | cut -d ' ' -f 3 | cut -d \" -f 2)"; \
		cargo publish; \
		git tag -a "$$NAME-$$VER" -m "$$VER"; \
		cd ../.. ; \
		)
