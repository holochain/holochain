{ pkgs }:
let
  t-test = pkgs.writeShellScriptBin "hc-test"
  ''
  set -euxo pipefail
  RUST_BACKTRACE=1 \
  cargo test -- --nocapture
  '';

  t-merge = pkgs.writeShellScriptBin "hc-merge-test"
  ''
  set -euxo pipefail
  RUST_BACKTRACE=1 \
  hn-rust-fmt-check \
  && hn-rust-clippy \
  && hc-test \
  && hc-test-wasm
  '';

  t-cover = pkgs.writeShellScriptBin "hc-coverage-test"
  ''
  set -euxo pipefail

  # kcov does not work with the global /holochain-rust/target
  mkdir -p target

  # actually kcov does not work with workspace target either
  # we need to use targets in each crate - but that is slow
  # use symlinks so we don't have to recompile deps over and over
  for i in ''$(find crates -maxdepth 1 -mindepth 1 -type d | sort); do
    # skip some crates that aren't ready yet
    case "$i" in
      "crates/dpki" | "crates/keystore" | "crates/legacy" | "crates/lib3h_protocol")
        continue
        ;;
    esac

    # delete all other test binaries so they don't get run multiple times
    rm -rf $(find target/debug -maxdepth 1 -mindepth 1 -type f)

    echo "-------"
    echo "coverage for '$i'"
    echo "-------"

    # ensure we use the shared target dir
    export CARGO_TARGET_DIR=$(readlink -f ./target)

    # cd into crate dir
    # create temporary local target symlink
    # build the test binaries
    # run the code coverage
    # remove the temporary local target symlink
    (
      cd $i && \
      rm -rf target && \
      ln -s ../../target target && \
      cargo test --no-run && \
      cargo make coverage-kcov && \
      rm -rf target
    )
  done

  # we cannot do codecov.io right now with the private repo
  # so we'll just open the coverage report in a browser
  xdg-open target/coverage/index.html
  '';

  maybe_linux = if pkgs.stdenv.isLinux then [ t-cover ] else [ ];
in
{
  buildInputs = [ t-test t-merge ] ++ maybe_linux;
}
