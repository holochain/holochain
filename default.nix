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
 ) { config = config; };
 # END HOLONIX IMPORT BOILERPLATE

in
with holonix.pkgs;
{
 dev-shell = stdenv.mkDerivation (holonix.shell // {
  name = "dev-shell";

  shellHook = holonix.pkgs.lib.concatStrings [
   holonix.shell.shellHook
   ''
    source .env
    export HC_TARGET_PREFIX=$NIX_ENV_PREFIX
    export CARGO_TARGET_DIR="$HC_TARGET_PREFIX/target"
    export CARGO_CACHE_RUSTC_INFO=1

    export HC_WASM_CACHE_PATH="$HC_TARGET_PREFIX/.wasm_cache"
    mkdir -p $HC_WASM_CACHE_PATH

    export PEWPEWPEW_PORT=4343
   ''
  ];

  buildInputs = [
   holonix.pkgs.gnuplot
   holonix.pkgs.flamegraph
   holonix.pkgs.ngrok
   holonix.pkgs.jq
  ]
   ++ holonix.shell.buildInputs

   # release hooks
   ++ (holonix.pkgs.callPackage ./release {
    pkgs = holonix.pkgs;
    config = config;
   }).buildInputs

   # main test script
   ++ (holonix.pkgs.callPackage ./test {
    pkgs = holonix.pkgs;
   }).buildInputs

   # convenience command for executing dna-util
   # until such time as we have release artifacts
   # that can be built directly as nix packages
   ++ ([(
    holonix.pkgs.writeShellScriptBin "dna-util" ''
    cargo run --manifest-path "''${HC_TARGET_PREFIX}/crates/dna_util/Cargo.toml" -- "''${@}"
    ''
   )])

   ++ ([(
    holonix.pkgs.writeShellScriptBin "hc-bench" ''
    cargo bench --bench bench
    '')])

   ++ ([(
    holonix.pkgs.writeShellScriptBin "hc-bench-github" ''
    set -x

    commit=''${1}
    compare=develop
    token=''${2}
    dir="$TMP/$commit"
    tarball="$dir/tarball.tar.gz"
    github_url="https://github.com/Holo-Host/holochain/archive/$commit.tar.gz"

    mkdir -p $dir
    curl -L --cacert $SSL_CERT_FILE -H "Authorization: token $token" $github_url > $tarball
    tar -zxvf $tarball -C $dir
    cd $dir/holochain-*
    CARGO_TARGET_DIR=$BENCH_OUTPUT_DIR cargo bench --bench bench -- --save-baseline $compare
    CARGO_TARGET_DIR=$BENCH_OUTPUT_DIR cargo bench --bench bench -- --save-baseline $commit
    # CARGO_TARGET_DIR=$BENCH_OUTPUT_DIR cargo bench --bench bench -- --baseline $compare --load-baseline $commit

    jq -n --arg report "\`\`\`$( CARGO_TARGET_DIR=$BENCH_OUTPUT_DIR cargo bench --bench bench -- --baseline $compare --load-baseline $commit )\`\`\`" '{body: $report}' | curl -L -X POST -H "Accept: application/vnd.github.v3+json" --cacert $SSL_CERT_FILE -H "Authorization: token $token" https://api.github.com/repos/Holo-Host/holochain/commits/$commit/comments -d@-
    '')])

    ++ ([(
     holonix.pkgs.writeShellScriptBin "pewpewpew" ''
     # compile and build pewpewpew
     ( cd crates/pewpewpew && cargo run )
     '')])

    ++ ([(
     holonix.pkgs.writeShellScriptBin "pewpewpew-ngrok" ''
     # serve up a local pewpewpew instance that github can point to for testing
     ngrok http http://127.0.0.1:$PEWPEWPEW_PORT
    '')])

    ++ ([(
     holonix.pkgs.writeShellScriptBin "pewpewpew-gen-secret" ''
     # generate a new github secret
     cat /dev/urandom | head -c 64 | base64
    '')])
  ;
 });
}
