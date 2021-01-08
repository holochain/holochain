{ lib
, stdenv
, mkShell
, rustup
, coreutils
, darwin

, holonix
, hcRustPlatform
, hcToplevelDir
, hcTargetPrefixEval
, pkgs
}:

let
  inherit (lib.attrsets) mapAttrsToList mapAttrs;

  commonShellHook = ''
    ${hcTargetPrefixEval}
    echo Using "$HC_TARGET_PREFIX" as target prefix...

    export CARGO_TARGET_DIR="$HC_TARGET_PREFIX/target"
    export CARGO_CACHE_RUSTC_INFO=1
    export CARGO_HOME="$HC_TARGET_PREFIX/.cargo"
    export CARGO_INSTALL_ROOT="$HC_TARGET_PREFIX/.cargo"
    # FIXME: we currently rely on lair-keystore being installed and found here by `holochain`
    export PATH="$PATH:$CARGO_INSTALL_ROOT/bin"

    export HC_TEST_WASM_DIR="$HC_TARGET_PREFIX/.wasm_target"
    mkdir -p $HC_TEST_WASM_DIR

    export HC_WASM_CACHE_PATH="$HC_TARGET_PREFIX/.wasm_cache"
    mkdir -p $HC_WASM_CACHE_PATH

    export RUSTFLAGS="${holonix.rust.compile.stable-flags}"
  ''
    # TODO: make thinlto linking work on stable
    # export RUSTFLAGS="$RUSTFLAGS -C linker-plugin-lto -C linker=${holonix.pkgs.clang_10}/bin/clang -C link-arg=-fuse-ld=${holonix.pkgs.lld}/bin/lld"
    ;

  commonPkgsInputs = [] ++ lib.optional stdenv.isDarwin [ darwin.apple_sdk.frameworks.AppKit ];

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
      ++ commonPkgsInputs
      ++ devPkgsLists.core
      ;
    shellHook = commonShellHook;
  };

  # we may need more packages on CI
  ci = coreDev;

  happDev = mkShell {
    nativeBuildInputs = applicationPkgsInputs.nativeBuild;
    buildInputs = applicationPkgsInputs.build
      ++ commonPkgsInputs
      ++ devPkgsLists.happ
      # ++ lib.optional stdenv.isDarwin [
      #   darwin.apple_sdk.frameworks.AppKit
      # ]
      ;
    shellHook = commonShellHook;
  };

  coreDevRustup = coreDev.overrideAttrs (attrs: {
    buildInputs = attrs.buildInputs ++ [
      rustup
    ];
  });
}
