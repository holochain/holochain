SHELL		= /bin/bash

.PHONY: all test
all:		test

# test -- build required WASM and execute unit tests
test:
	cargo build --features 'build_wasms' --manifest-path=crates/holochain/Cargo.toml
	cargo test

# nix-test, ...
#
# Provides a nix-shell environment, and runs the desired Makefile target.
nix-%:
	nix-shell --pure --run "make $*"
