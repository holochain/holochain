{ stdenv
, callPackage
, lib
, writeShellScriptBin

, holonix
, holonixPath
, hcToplevelDir
, releaseAutomation
, util-linux
, iproute2
}:

rec {
  hcTest = writeShellScriptBin "hc-test" ''
    set -euxo pipefail
    export RUST_BACKTRACE=1

    hc-test-standard
    hc-test-slow
    hc-test-wasm
  '';

  hcStandardTests = writeShellScriptBin "hc-test-standard" ''
    set -euxo pipefail
    export RUST_BACKTRACE=1

    # run all the non-slow cargo tests
    cargo build --features 'build' -p holochain_wasm_test_utils
    cargo test ''${CARGO_TEST_ARGS:-} --workspace --features slow_tests,glacial_tests,test_utils,build_wasms,db-encryption --lib --tests --profile fast-test ''${1-} -- --nocapture
  '';

  hcStandardTestsNextest = writeShellScriptBin "hc-test-standard-nextest" ''
    set -euxo pipefail
    export RUST_BACKTRACE=1

    # run all the cargo tests with no net
    cargo build --features 'build' -p holochain_wasm_test_utils
    cargo nextest ''${CARGO_NEXTEST_ARGS:-run --test-threads=2} --workspace --features slow_tests,glacial_tests,test_utils,build_wasms,db-encryption --lib --tests --cargo-profile fast-test skip-all-tests
    ${util-linux}/bin/unshare -n -r bash -c "${iproute2}/bin/ip link set lo up && ip addr show lo && cargo nextest ''${CARGO_NEXTEST_ARGS:-run --test-threads=2} --workspace --features slow_tests,glacial_tests,test_utils,build_wasms,db-encryption --lib --tests --cargo-profile fast-test ''${1-}"
  '';

  hcWasmTests = writeShellScriptBin "hc-test-wasm" ''
    set -euxo pipefail
    export RUST_BACKTRACE=1

    # run all the wasm tests (within wasm) with the conductor mocked
    cargo test ''${CARGO_TEST_ARGS:-} --lib --manifest-path=crates/test_utils/wasm/wasm_workspace/Cargo.toml --all-features -- --nocapture
  '';

  hcReleaseAutomationTest = writeShellScriptBin "hc-test-release-automation" ''
    set -euxo pipefail
    export RUST_BACKTRACE=1

    # make sure the binary is built
    cargo build --locked --manifest-path=crates/release-automation/Cargo.toml
    # run the release-automation tests
    cargo test ''${CARGO_TEST_ARGS:-} --locked --manifest-path=crates/release-automation/Cargo.toml ''${@}
  '';

  hcReleaseAutomationTestRepo =
    let
      prepareWorkspaceCmd = ''
        rm -rf ''${TEST_WORKSPACE:?}
        git clone $PWD ''${TEST_WORKSPACE:?}
      '';

      crateCmd = logLevel: ''
        ${releaseAutomation} \
            --workspace-path=''${TEST_WORKSPACE:?} \
            --log-level=${logLevel} \
          crate \
            apply-dev-versions \
            --commit \
            --no-verify
      '';
      releaseCmd = logLevel: ''
        ${releaseAutomation} \
            --workspace-path=''${TEST_WORKSPACE:?} \
            --log-level=${logLevel} \
            --match-filter="^(holochain|holochain_cli|kitsune_p2p_proxy)$" \
          release \
            --no-verify-pre \
            --force-branch-creation \
            --disallowed-version-reqs=">=0.1" \
            --allowed-matched-blockers=UnreleasableViaChangelogFrontmatter \
            --steps=CreateReleaseBranch,BumpReleaseVersions
      '';
    in
    writeShellScriptBin "hc-test-release-automation-repo" ''
      set -euxo pipefail

      export TEST_WORKSPACE=$(mktemp -d)
      if [[ "''${KEEP_TEST_WORKSPACE:-false}" != "true" ]]; then
        trap "rm -rf ''${TEST_WORKSPACE:?}" EXIT
      fi

      # check the state of the repository
      (
        ${prepareWorkspaceCmd}
        ${crateCmd "debug"}
        ${releaseCmd "debug"}
      )
    '';

  hcStaticChecks =
    let
      pathPrefix = lib.makeBinPath
        (builtins.attrValues {
          inherit (holonix.pkgs)
            hnRustClippy
            hnRustFmtCheck
            hnRustFmtFmt
            ;
        })
      ;
    in
    writeShellScriptBin "hc-static-checks" ''
      export PATH=${pathPrefix}:$PATH

      set -euxo pipefail
      export RUST_BACKTRACE=1
      hn-rust-fmt-check
      hn-rust-clippy
    '';

  hcMergeTest = writeShellScriptBin "hc-merge-test" ''
    set -euxo pipefail
    export RUST_BACKTRACE=1
    hc-static-checks
    hc-test
  '';

  hcReleaseTest = writeShellScriptBin "hc-test-release" ''
    set -euxo pipefail
    export RUST_BACKTRACE=1

    ${hcReleaseAutomationTest}/bin/hc-test-release-automation
    ${hcReleaseAutomationTestRepo}/bin/hc-test-release-automation-repo
  '';

  hcSpeedTest = writeShellScriptBin "hc-speed-test" ''
    cargo test speed_test_prep --test speed_tests --release --manifest-path=crates/holochain/Cargo.toml --features "build_wasms" -- --ignored
    cargo test speed_test_all --test speed_tests --release --manifest-path=crates/holochain/Cargo.toml --features "build_wasms" -- --ignored --nocapture
  '';

  hcFlakyTest = writeShellScriptBin "hc-flaky-test" ''
    set -euxo pipefail
    export RUST_BACKTRACE=1

    for i in {0..100}
    do
      cargo test --manifest-path=crates/holochain/Cargo.toml --features slow_tests,glacial_tests,build_wasms -- --nocapture
    done
    for i in {0..100}
    do
      cargo test --workspace --exclude holochain -- --nocapture
    done
    for i in {0..100}
    do
      cargo test --lib --manifest-path=crates/test_utils/wasm/wasm_workspace/Cargo.toml --all-features -- --nocapture
    done
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

    echo "hc binary installation"
    command -v hc
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

  hcRegenReadmes = writeShellScriptBin "hc-regen-readmes" ''
    cargo-readme readme --project-root=crates/release-automation/ --output=README.md;
  '';

  hcRegenNixExpressions = writeShellScriptBin "hc-regen-nix-expressions" ''
    set -xe
    pushd ${hcToplevelDir}
    crate2nix generate \
        -f crates/release-automation/Cargo.toml \
        -o crates/release-automation/Cargo.nix
    git commit crates/release-automation/Cargo.nix -m "chore: hc-regen-nix-expressions"
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
