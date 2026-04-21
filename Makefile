# holochain Makefile

# All default features of binaries excluding mutually exclusive features wasmer-sys-cranelift & wasmer-wasmi
# and tx5 transport and iroh transport
COMMON_DEFAULT_FEATURES=slow_tests,build_wasms,sqlite-encrypted
DEFAULT_FEATURES=transport-iroh,$(COMMON_DEFAULT_FEATURES)
DEFAULT_FEATURES_TRANSPORT_TX5=transport-tx5-backend-go-pion,$(COMMON_DEFAULT_FEATURES)
UNSTABLE_FEATURES=unstable-sharding,unstable-functions,unstable-migration,$(DEFAULT_FEATURES)

# mark everything as phony because it doesn't represent a file-system output
.PHONY: default \
	static-all static-fmt static-toml static-clippy static-clippy-unstable \
	static-doc build-workspace-wasmer-sys-cranelift build-workspace-wasmer-wasmi \
	build-workspace-wasmer-sys-llvm test-workspace-wasmer-sys-cranelift \
	test-workspace-wasmer-sys-llvm test-workspace-wasmer-wasmi \
	build-workspace-wasmer-sys-cranelift-unstable \
	test-workspace-wasmer-sys-cranelift-unstable \
	toml-fix

# default to running everything (first rule)
default: build-workspace-wasmer-sys-cranelift \
	test-workspace-wasmer-sys-cranelift \
	build-workspace-wasmer_wasmi \
	test-workspace-wasmer_wasmi

# execute all static code validation
static-all: static-fmt static-toml static-clippy static-clippy-unstable static-doc

# ensure committed code is formatted properly
static-fmt:
	cargo fmt --check

# lint our toml files
static-toml:
	cargo install taplo-cli@0.10.0
	taplo format --check ./*.toml
	taplo format --check ./crates/**/*.toml

# fix our toml files
toml-fix:
	cargo install taplo-cli@0.10.0
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
build-workspace-wasmer-sys-cranelift:
	cargo build \
		--workspace \
		--locked \
		--all-targets \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer-sys-cranelift

build-workspace-wasmer-sys-cranelift-unstable:
	cargo build \
		--workspace \
		--locked \
		--all-targets \
		--no-default-features \
		--features $(UNSTABLE_FEATURES),wasmer-sys-cranelift

build-workspace-wasmer-sys-llvm:
	cargo build \
		--workspace \
		--locked \
		--all-targets \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer-sys-llvm

build-workspace-wasmer-wasmi:
	cargo build \
		--workspace \
		--locked \
		--all-targets \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer-wasmi

build-workspace-wasmer-sys-cranelift-transport_tx5:
	cargo build \
		--workspace \
		--locked \
		--all-targets \
		--no-default-features \
		--features $(DEFAULT_FEATURES_TRANSPORT_TX5),wasmer-sys-cranelift

# execute tests on all crates with the cranelift wasmer compiler and iroh transport
test-workspace-wasmer-sys-cranelift:
	RUST_BACKTRACE=1 cargo nextest run \
		--workspace \
		--locked \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer-sys-cranelift

# execute tests on all crates with the LLVM wasmer compiler and iroh transport
test-workspace-wasmer-sys-llvm:
	RUST_BACKTRACE=1 cargo nextest run \
		--workspace \
		--locked \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer-sys-llvm

# executes tests on all crates with wasmer compiler
test-workspace-wasmer-sys-cranelift-unstable:
	RUST_BACKTRACE=1 cargo nextest run \
		--workspace \
		--locked \
		--no-default-features \
		--features $(UNSTABLE_FEATURES),wasmer-sys-cranelift

# execute tests on all crates with wasmer interpreter
test-workspace-wasmer-wasmi:
	RUST_BACKTRACE=1 cargo nextest run \
		--workspace \
		--locked \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer-wasmi

# execute tests on all crates with wasmer compiler and tx5 transport
test-workspace-wasmer-sys-cranelift-transport_tx5:
	RUST_BACKTRACE=1 cargo nextest run \
		--workspace \
		--locked \
		--no-default-features \
		--features $(DEFAULT_FEATURES_TRANSPORT_TX5),wasmer-sys-cranelift

clean:
	cargo clean
    # Remove untracked .dna files
	git ls-files -z --others --ignored --exclude-standard -- '*.dna' | xargs -0 rm --
    # Remove untracked .happ files
	git ls-files -z --others --ignored --exclude-standard '*.happ' | xargs -0 rm --
