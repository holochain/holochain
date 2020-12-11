{ lib
, stdenv
, mkShell
, rustup

, holonix
, hcRustPlatform
, hcToplevelDir
, pkgs
}:

let
  inherit (lib.attrsets) mapAttrsToList;

  commonShellHook = ''
    export HC_TARGET_PREFIX=''${NIX_ENV_PREFIX:-${builtins.toString hcToplevelDir}}
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

  devPkgsList = builtins.attrValues pkgs.dev;

  happDevFn = { includeRust ? true }: mkShell {
    buildInputs = builtins.attrValues (
      pkgs.applications // (
        if includeRust
        then hcRustPlatform.rust
        else {}
      )
    );
    shellHook = commonShellHook;
  };
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
      ++ devPkgsList
      ;
    shellHook = commonShellHook;
  };

  # we may need more packages on CI
  ci = coreDev;

  happDev = happDevFn {
    includeRust = true;
  };

  happDevRustExcluded = happDevFn {
    includeRust = false;
  };

  coreDevRustup = coreDev.overrideAttrs (attrs: {
    buildInputs = attrs.buildInputs ++ [
      rustup
    ];
  });
}
