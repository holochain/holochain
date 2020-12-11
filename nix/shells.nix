{ lib
, stdenv
, mkShell
, rustup
, coreutils

, holonix
, hcRustPlatform
, hcToplevelDir
, pkgs
}:

let
  inherit (lib.attrsets) mapAttrsToList mapAttrs;

  commonShellHook = ''
    if [[ -n "$NIX_ENV_PREFIX" ]]; then
      export HC_TARGET_PREFIX="$NIX_ENV_PREFIX"
    elif test -d "${builtins.toString hcToplevelDir}" &&
         test -w "${builtins.toString hcToplevelDir}"; then
      export HC_TARGET_PREFIX="${builtins.toString hcToplevelDir}"
    elif test -d "$HOME" && test -w "$HOME"; then
      export HC_TARGET_PREFIX="$HOME/.cache/holochain-dev"
      mkdir -p "$HC_TARGET_PREFIX"
    else
      export HC_TARGET_PREFIX="$(${coreutils}/bin/mktemp -d)"
    fi
    echo Using "$HC_TARGET_PREFIX" as target prefix...

    export CARGO_TARGET_DIR="''${HC_TARGET_PREFIX}/target"
    export HC_TEST_WASM_DIR="''${HC_TARGET_PREFIX}/.wasm_target"
    mkdir -p $HC_TEST_WASM_DIR
    export CARGO_CACHE_RUSTC_INFO=1

    export HC_WASM_CACHE_PATH="$HC_TARGET_PREFIX/.wasm_cache"
    mkdir -p $HC_WASM_CACHE_PATH

    export RUSTFLAGS="${holonix.rust.compile.stable-flags}"
  ''
    # TODO: make thinlto linking work on stable
    # export RUSTFLAGS="$RUSTFLAGS -C linker-plugin-lto -C linker=${holonix.pkgs.clang_10}/bin/clang -C link-arg=-fuse-ld=${holonix.pkgs.lld}/bin/lld"
    ;

  applicationPkgsInputs = {
    build = mapAttrsToList (name: value:
      value.buildInputs
    ) pkgs.applications;

    nativeBuild = mapAttrsToList (name: value:
      value.nativeBuildInputs
    ) pkgs.applications;
  };

  devPkgsLists =
    mapAttrs (name: value:
      mapAttrsToList (name': value':
        value'
      ) value
    ) pkgs.dev;
in

rec {
  # TODO: after downsizing holonix.shell, refactor this and use it as a foundation for all the following
  # legacy = stdenv.mkDerivation (holonix.shell // {
  #   shellHook = lib.concatStrings [
  #     holonix.shell.shellHook
  #     commonShellHook
  #   ];

  # TODO: clarify if these are still needed by anything/anyone
  #   buildInputs = with holonix.pkgs; [
  #       gnuplot
  #       flamegraph
  #       fd
  #       ngrok
  #       jq
  #     ]
  #     ++ holonix.shell.buildInputs
  #     ++ devPkgsList
  #     ;
  # });

  coreDev = mkShell {
    nativeBuildInputs = applicationPkgsInputs.nativeBuild;
    buildInputs = applicationPkgsInputs.build
      ++ devPkgsLists.core
      ;
    shellHook = commonShellHook;
  };

  # we may need more packages on CI
  ci = coreDev;

  happDev = mkShell {
    nativeBuildInputs = applicationPkgsInputs.nativeBuild;
    buildInputs = applicationPkgsInputs.build
      # ++ devPkgsLists.core
      ++ devPkgsLists.happ
      ;
    shellHook = commonShellHook;
  };

  coreDevRustup = coreDev.overrideAttrs (attrs: {
    buildInputs = attrs.buildInputs ++ [
      rustup
    ];
  });
}
