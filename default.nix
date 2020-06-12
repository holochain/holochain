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
    export HC_TARGET_PREFIX=$NIX_ENV_PREFIX
    export CARGO_TARGET_DIR="$HC_TARGET_PREFIX/target"
    export CARGO_CACHE_RUSTC_INFO=1
   ''
  ];

  buildInputs = [
   holonix.pkgs.gnuplot
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
  ;
 });
}
