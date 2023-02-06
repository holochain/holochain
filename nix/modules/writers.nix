{
  perSystem = { config, lib, ... }:
    let
      pkgs = config.pkgs;
    in
    {
      options.writers = {
        writePureShellScript = lib.mkOption {
          type = lib.types.functionTo lib.types.anything;
        };
        writePureShellScriptBin = lib.mkOption {
          type = lib.types.functionTo lib.types.anything;
        };
      };


      /*
        create a script that runs in a `pure` environment, in the sense that:
      - PATH only contains exactly the packages passed via the PATH arg
      - NIX_PATH is set to the path of the current `pkgs`
      - TMPDIR is set up and cleaned up even if the script fails
      - all environment variables are unset, except:
        - the ones listed in `keepVars` below
        - ones listed via the KEEP_VARS variable
      - the behavior is similar to `nix-shell --pure`
      */
      config.writers =
        let
          mkScript = PATH: script: ''
            #!${pkgs.bash}/bin/bash
            set -Eeuo pipefail

            export PATH="${lib.makeBinPath PATH}"
            export NIX_PATH=nixpkgs=${pkgs.path}

            TMPDIR=$(${pkgs.coreutils}/bin/mktemp -d)

            trap '${pkgs.coreutils}/bin/chmod -R +w $TMPDIR;  ${pkgs.coreutils}/bin/rm -rf "$TMPDIR"' EXIT

            if [ -z "''${IMPURE:-}" ]; then
              ${cleanEnv}
            fi

            ${script}
          '';

          # list taken from nix source: src/nix-build/nix-build.cc
          keepVars = lib.concatStringsSep " " [
            "HOME"
            "XDG_RUNTIME_DIR"
            "USER"
            "LOGNAME"
            "DISPLAY"
            "WAYLAND_DISPLAY"
            "WAYLAND_SOCKET"
            "PATH"
            "TERM"
            "IN_NIX_SHELL"
            "NIX_SHELL_PRESERVE_PROMPT"
            "TZ"
            "PAGER"
            "NIX_BUILD_SHELL"
            "SHLVL"
            "http_proxy"
            "https_proxy"
            "ftp_proxy"
            "all_proxy"
            "no_proxy"

            # We want to keep out own variables as well
            "IMPURE"
            "KEEP_VARS"
            "NIX_PATH"
          ];

          cleanEnv = ''

        KEEP_VARS="''${KEEP_VARS:-}"

        unsetVars=$(
          PATH=${pkgs.coreutils}/bin:${pkgs.findutils}/bin \
            comm \
              <(env | cut -d = -f 1 | sort) \
              <(echo "${keepVars} $KEEP_VARS" | tr " " "\n" | sort) \
              -2 \
              -3
        )

        echo -e "unsetting ENV variables:\n$(echo $unsetVars | tr "\n" " ")"
        unset $unsetVars
      '';

        in
        {
          writePureShellScript = PATH: script:
            pkgs.writeScript "script.sh" (mkScript PATH script);

          writePureShellScriptBin = binName: PATH: script:
            pkgs.writeScriptBin binName (mkScript PATH script);
        };
    };
}

