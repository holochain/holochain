{ stdenv
, callPackage
, lib
, writeShellScriptBin

, holonixPath
, hcToplevelDir
, hcRunCrate
}:

rec {
  inherit hcRunCrate;

  # TODO: potentially remove these
  hnRustClippy = builtins.elemAt (callPackage "${holonixPath}/rust/clippy" {}).buildInputs 0;
  hnRustFmtCheck = builtins.elemAt (callPackage "${holonixPath}/rust/fmt/check" {}).buildInputs 0;
  hnRustFmtFmt = builtins.elemAt (callPackage "${holonixPath}/rust/fmt/fmt" {}).buildInputs 0;

  hcTest = writeShellScriptBin "hc-test" ''
    set -euxo pipefail
    export RUST_BACKTRACE=1

    ${lib.optionalString stdenv.isDarwin ''
    # fix for "too many open files" that breaks tokio and lmdb
    ulimit -n 10240
    ''}

    # ensure plain build works
    cargo build --no-default-features --manifest-path=crates/holochain/Cargo.toml

    # alas, we cannot specify --features in the virtual workspace
    cargo test warm_wasm_tests --manifest-path=crates/holochain/Cargo.toml --features slow_tests,build_wasms
    cargo test --manifest-path=crates/holochain/Cargo.toml --features slow_tests -- --nocapture
  '';

  hcMergeTest = let
      pathPrefix = lib.makeBinPath [
        hcTest
        hnRustClippy
        hnRustFmtCheck
      ];
    in writeShellScriptBin "hc-merge-test" ''
    export PATH=${pathPrefix}:$PATH

    set -euxo pipefail
    export RUST_BACKTRACE=1
    hn-rust-fmt-check
    hn-rust-clippy
    hc-test
  '';

  hcSpeedTest = writeShellScriptBin "hc-speed-test" ''
    cargo test speed_test_prep --test speed_tests --release --manifest-path=crates/holochain/Cargo.toml --features "build_wasms" -- --ignored
    cargo test speed_test_all --test speed_tests --release --manifest-path=crates/holochain/Cargo.toml --features "build_wasms" -- --ignored --nocapture
  '';

  hcDoctor = writeShellScriptBin "hc-doctor" ''
    echo "### holochain doctor ###"
    echo

    echo "if you have installed holochain directly using hc-install it should be in the cargo root"
    echo "if that is what you want it may be worth running hc-install to 'refresh' it as HEAD moves quickly"
    echo
    echo "if you are using the more stable binaries provided by holonix it should be in /nix/store/../bin"
    echo

    echo "cargo install root:"
    echo $CARGO_INSTALL_ROOT
    echo

    echo "holochain binary installation:"
    command -v holochain
    echo

    echo "dna-util binary installation"
    command -v dna-util
    echo
  '';

  hcBench = writeShellScriptBin "hc-bench" ''
    cargo bench --bench bench
  '';

  hcFmtAll = writeShellScriptBin "hc-fmt-all" ''
    fd Cargo.toml crates | xargs -L 1 cargo fmt --manifest-path
  '';

  hcBenchGithub = writeShellScriptBin "hc-bench-github" ''
    set -x

    # the first arg is the authentication token for github
    # @todo this is only required because the repo is currently private
    token=''${1}

    # set the target dir to somewhere it is less likely to be accidentally deleted
    CARGO_TARGET_DIR=$BENCH_OUTPUT_DIR

    # run benchmarks from a github archive based on any ref github supports
    # @param ref: the github ref to benchmark
    function bench {

      ## vars
      ref=$1
      dir="$TMP/$ref"
      tarball="$dir/tarball.tar.gz"

      ## process

      ### fresh start
      mkdir -p $dir
      rm -f $dir/$tarball

      ### fetch code to bench
      curl -L --cacert $SSL_CERT_FILE -H "Authorization: token $token" "https://github.com/holochain/holochain/archive/$ref.tar.gz" > $tarball
      tar -zxvf $tarball -C $dir

      ### bench code
      cd $dir/holochain-$ref
      cargo bench --bench bench -- --save-baseline $ref

    }

    # load an existing report and push it as a comment to github
    function add_comment_to_commit {
      ## convert the report to POST-friendly json and push to github comment API
      jq \
      -n \
      --arg report \
      "\`\`\`$( cargo bench --bench bench -- --baseline $1 --load-baseline $2 )\`\`\`" \
      '{body: $report}' \
      | curl \
      -L \
      --cacert $SSL_CERT_FILE \
      -H "Authorization: token $token" \
      -X POST \
      -H "Accept: application/vnd.github.v3+json" \
      https://api.github.com/repos/holochain/holochain/commits/$2/comments \
      -d@-
    }

    commit=''${2}
    bench $commit

    # @todo make this flexible based on e.g. the PR base on github
    compare=develop
    bench $compare
    add_comment_to_commit $compare $commit
  '';
} // (if stdenv.isLinux then {
  hcCoverageTest = writeShellScriptBin "hc-coverage-test" ''
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
} else { })
