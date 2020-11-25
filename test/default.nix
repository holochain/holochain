{ pkgs }:
let
  t-test = pkgs.writeShellScriptBin "hc-test"
  ''
  set -euxo pipefail
  cargo test warm_wasm_tests --manifest-path=crates/holochain/Cargo.toml --features "slow_tests build_wasms"
  RUST_BACKTRACE=1 \
  cargo test -- --nocapture

  # we need some output so circle doesn't break
  # print a line every minute
  if [ ! -z ''${CIRCLECI+x} ]; then
   for i in $(seq 60); do
     echo "tick still testing ($i)"
     sleep 60
   done &
   _jid=$!
  fi;

  # alas, we cannot specify --features in the virtual workspace
  cargo test --manifest-path=crates/holochain/Cargo.toml --features slow_tests -- --nocapture

  # stop our background ticker
  if [ ! -z ''${CIRCLECI+x} ]; then
   kill $_jid
  fi
  '';

  t-merge = pkgs.writeShellScriptBin "hc-merge-test"
  ''
  set -euxo pipefail
  RUST_BACKTRACE=1 \
  hn-rust-fmt-check \
  && hn-rust-clippy \
  && hc-test
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
      "crates/dpki" | "crates/keystore" )
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

  t-speed = pkgs.writeShellScriptBin "hc-speed-test"
  ''
  cargo test speed_test_prep --test speed_tests --release --manifest-path=crates/holochain/Cargo.toml --features "build_wasms" -- --ignored
  cargo test speed_test_all --test speed_tests --release --manifest-path=crates/holochain/Cargo.toml --features "build_wasms" -- --ignored --nocapture
  '';

  maybe_linux = if pkgs.stdenv.isLinux then [ t-cover ] else [ ];
in
{
  buildInputs = [ t-test t-merge t-speed ] ++ maybe_linux;
}
