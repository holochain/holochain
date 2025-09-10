# holochain Makefile

# All default features of binaries excluding mutually exclusive features wasmer_sys & wasmer_wamr
DEFAULT_FEATURES=slow_tests,build_wasms,sqlite-encrypted,hc_demo_cli/build_demo
UNSTABLE_FEATURES=chc,unstable-sharding,unstable-warrants,unstable-functions,unstable-countersigning,unstable-migration,$(DEFAULT_FEATURES)

# mark everything as phony because it doesn't represent a file-system output
.PHONY: default \
	static-all static-fmt static-toml static-clippy static-clippy-unstable \
	static-doc build-workspace-wasmer_sys build-workspace-wasmer_wamr \
	test-workspace-wasmer_sys test-workspace-wasmer_wamr \
	build-workspace-wasmer_sys-unstable \
	test-workspace-wasmer_sys-unstable \
	toml-fix

# default to running everything (first rule)
default: build-workspace-wasmer_sys \
	test-workspace-wasmer_sys \
	build-workspace-wasmer_wamr \
	test-workspace-wasmer_wamr

# execute all static code validation
static-all: static-fmt static-toml static-clippy static-clippy-unstable static-doc

# ensure committed code is formatted properly
static-fmt:
	cargo fmt --check

# lint our toml files
static-toml:
	cargo install taplo-cli@0.9.0
	taplo format --check ./*.toml
	taplo format --check ./crates/**/*.toml

# fix our toml files
toml-fix:
	cargo install taplo-cli@0.9.0
	taplo format ./*.toml
	taplo format ./crates/**/*.toml

# ensure our chosen style lints are followed
static-clippy:
	CHK_SQL_FMT=1 cargo clippy --all-targets --features $(DEFAULT_FEATURES)

static-clippy-unstable:
	CHK_SQL_FMT=1 cargo clippy --all-targets --features $(UNSTABLE_FEATURES)

# ensure we can build the docs
static-doc:
	RUSTDOCFLAGS=-Dwarnings cargo doc

# build all targets
# this not only builds the test binaries for usage by `test-workspace`,
# but also ensures targets like benchmarks remain buildable.
# NOTE: excludes must match test-workspace nextest params,
#       otherwise some rebuilding will occur due to resolver = "2"
build-workspace-wasmer_sys:
	cargo build \
		--workspace \
		--locked \
		--all-targets \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer_sys

build-workspace-wasmer_sys-unstable:
	cargo build \
		--workspace \
		--locked \
		--all-targets \
		--no-default-features \
		--features $(UNSTABLE_FEATURES),wasmer_sys

build-workspace-wasmer_wamr:
	cargo build \
		--workspace \
		--locked \
		--all-targets \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer_wamr

# execute tests on all crates with wasmer compiler
test-workspace-wasmer_sys:
	RUST_BACKTRACE=1 cargo nextest run \
		--workspace \
		--locked \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer_sys \
		$(if $(PARTITION_COUNT), --partition count:$(PARTITION)/$(PARTITION_COUNT),)

# executes tests on all crates with wasmer compiler
test-workspace-wasmer_sys-unstable:
	RUST_BACKTRACE=1 cargo nextest run \
		--workspace \
		--locked \
		--no-default-features \
		--features $(UNSTABLE_FEATURES),wasmer_sys \
		$(if $(PARTITION_COUNT), --partition count:$(PARTITION)/$(PARTITION_COUNT),)

# execute tests on all crates with wasmer interpreter
test-workspace-wasmer_wamr:
	RUST_BACKTRACE=1 cargo nextest run \
		--workspace \
		--locked \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer_wamr \
		$(if $(PARTITION_COUNT), --partition count:$(PARTITION)/$(PARTITION_COUNT),)
