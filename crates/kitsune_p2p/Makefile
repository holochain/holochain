STATIC_CHECKS := static-fmt static-toml static-clippy static-doc

.PHONY: $(STATIC_CHECKS) format build build-each test

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
	CHK_SQL_FMT=1 cargo clippy --all-targets -- --deny warnings

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
