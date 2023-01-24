{ config, stdenv, mkShell, rust, rustc ? rust.packages.stable.rust.rustc

, callPackage, kcov, binutils, gcc, gnumake, openssl, pkgconfig, cargo-make
, curl }:
let rustConfig = import ./config.nix;

in mkShell {

  # https://github.com/NixOS/nixpkgs/blob/master/doc/languages-frameworks/rust.section.md
  buildInputs = [ binutils gcc gnumake openssl pkgconfig cargo-make curl rustc ]
    ++ (if stdenv.isLinux then [ kcov ] else [ ]);

  inputsFrom = builtins.map (path: callPackage path { }) [
    ./clippy
    ./fmt
    ./manifest
    ./flush
  ];

  shellHook = ''
    # non-nixos OS can have a "dirty" setup with rustup installed for the current
    # user.
    # `nix-shell` can inherit this e.g. through sourcing `.bashrc`.
    # even `nix-shell --pure` will still source some files and inherit paths.
    # for those users we can at least give the OS a clue that we want our pinned
    # rust version through this environment variable.
    # https://github.com/rust-lang/rustup.rs#environment-variables
    # https://github.com/NixOS/nix/issues/903
    export RUSTUP_TOOLCHAIN="${rustc.version}"
    # TODO: clarify if we want incremental builds in release mode, as they're enabled by default on non-release builds: https://github.com/rust-lang/cargo/pull/4817
    export CARGO_INCREMENTAL="${rustConfig.compile.incremental}"
    export RUST_LOG="${rustConfig.log}"
    export NUM_JOBS="${rustConfig.compile.jobs}"
    export RUST_BACKTRACE="${rustConfig.backtrace}"

    export RUSTFLAGS="${rustConfig.compile.stable-flags}"

    export CARGO_HOME="$NIX_ENV_PREFIX/.cargo"
    export CARGO_INSTALL_ROOT="$NIX_ENV_PREFIX/.cargo"
    export CARGO_TARGET_DIR="$NIX_ENV_PREFIX/target"
    export CARGO_CACHE_RUSTC_INFO=1
    export PATH="$CARGO_INSTALL_ROOT/bin:$PATH"
  '';
}
