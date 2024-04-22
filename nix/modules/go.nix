{ self
, lib
, inputs
, ...
} @ flake: {
  perSystem =
    { config
    , self'
    , inputs'
    , system
    , pkgs
    , ...
    }: {
      packages = {
        goWrapper =
          let
            go = pkgs.go_1_21;
          in
          # there is interference only in this specific case, we assemble a go derivationt that not propagate anything but still has everything available required for our specific use-case
            #
            # the wrapper inherits preconfigured environment variables from the
            # derivation that depends on the propagating go
          if pkgs.stdenv.isDarwin && pkgs.system == "x86_64-darwin" then
            pkgs.darwin.apple_sdk_11_0.stdenv.mkDerivation
              {
                name = "go";

                nativeBuildInputs = [
                  pkgs.makeBinaryWrapper
                  go
                ];

                dontBuild = true;
                dontUnpack = true;

                installPhase = ''
                  makeWrapper ${pkgs.go}/bin/go $out/bin/go \
                    ${builtins.concatStringsSep " " (
                      builtins.map (var: "--set ${var} \"\$${var}\"") 
                      [
                        "NIX_BINTOOLS_WRAPPER_TARGET_HOST_x86_64_apple_darwin"
                        "NIX_LDFLAGS"
                        "NIX_CFLAGS_COMPILE_FOR_BUILD"
                        "NIX_CFLAGS_COMPILE"

                        # confirmed needed above here

                        # unsure between here
                        # and here

                        # confirmed unneeded below here

                        # "NIX_CC"
                        # "NIX_CC_FOR_BUILD"
                        # "NIX_LDFLAGS_FOR_BUILD"
                        # "NIX_BINTOOLS"
                        # "NIX_CC_WRAPPER_TARGET_HOST_x86_64_apple_darwin"
                        # "NIX_CC_WRAPPER_TARGET_BUILD_x86_64_apple_darwin"
                        # "NIX_ENFORCE_NO_NATIVE"
                        # "NIX_DONT_SET_RPATH"
                        # "NIX_BINTOOLS_FOR_BUILD"
                        # "NIX_DONT_SET_RPATH_FOR_BUILD"
                        # "NIX_NO_SELF_RPATH"
                        # "NIX_IGNORE_LD_THROUGH_GCC"
                        # "NIX_PKG_CONFIG_WRAPPER_TARGET_HOST_x86_64_apple_darwin"
                        # "NIX_COREFOUNDATION_RPATH"
                        # "NIX_BINTOOLS_WRAPPER_TARGET_BUILD_x86_64_apple_darwin"
                      ]
                    )}
                '';
              }
          else go;
      };
    };
}
