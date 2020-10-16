# This is an example of what downstream consumers of holonix should do
# This is also used to dogfood as many commands as possible for holonix
# For example the release process for holonix uses this file
let

 # point this to your local config.nix file for this project
 # example.config.nix shows and documents a lot of the options
 config = import ./config.nix;

 # START HOLONIX IMPORT BOILERPLATE
 holonix = import (
  if ! config.holonix.use-github
  then config.holonix.local.path
  else fetchTarball {
   url = "https://github.com/${config.holonix.github.owner}/${config.holonix.github.repo}/tarball/${config.holonix.github.ref}";
   sha256 = config.holonix.github.sha256;
  }
 ) { config = config; use-stable-rust = true; };
 # END HOLONIX IMPORT BOILERPLATE

 callPackage = holonix.pkgs.callPackage;
 writeShellScriptBin = holonix.pkgs.writeShellScriptBin;

in {
  inherit holonix;

  pkgs = {
    # release hooks
    releaseHooks = callPackage ./release {
      pkgs = holonix.pkgs;
      config = config;
    };

    # main test script
    mainTestScript = callPackage ./test {
      pkgs = holonix.pkgs;
    };

    hcInstall = writeShellScriptBin "hc-install" ''
      hc-install-holochain
      hc-install-dna-util

      hc-doctor
    '';

    hcUninstall = writeShellScriptBin "hc-uninstall" ''
      hc-uninstall-holochain
      hc-uninstall-dna-util

      hc-doctor
    '';

    hcInstallHolochain = writeShellScriptBin "hc-install-holochain" ''
      cargo install --path crates/holochain
      echo 'holochain installed!'
      echo
    '';

    hcUninstallHolochain = writeShellScriptBin "hc-uninstall-holochain" ''
      cargo uninstall holochain
      echo 'holochain uninstalled!'
      echo
    '';

    hcInstallDnaUtil = writeShellScriptBin "hc-install-dna-util" ''
      cargo install --path crates/dna_util
      echo 'dna util installed!'
      echo
    '';
    
    hcUninstallDnaUtil = writeShellScriptBin "hc-uninstall-dna-util" ''
      cargo uninstall dna_util
      echo 'dna util uninstalled!'
      echo
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

    # convenience command for executing dna-util
    # until such time as we have release artifacts
    # that can be built directly as nix packages
    dnaUtil = writeShellScriptBin "dna-util" ''
      cargo run --manifest-path "''${HC_TARGET_PREFIX}/crates/dna_util/Cargo.toml" -- "''${@}"
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

    pewPewPew = writeShellScriptBin "pewpewpew" ''
      # compile and build pewpewpew
      ( cd crates/pewpewpew && cargo run )
    '';

    pewPewPewNgrok = writeShellScriptBin "pewpewpew-ngrok" ''
      # serve up a local pewpewpew instance that github can point to for testing
      ngrok http http://127.0.0.1:$PEWPEWPEW_PORT
    '';

    pewPewPewGenSecret = writeShellScriptBin "pewpewpew-gen-secret" ''
      # generate a new github secret
      cat /dev/urandom | head -c 64 | base64
    '';
  };
}
