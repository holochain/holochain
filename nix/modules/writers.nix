{
  perSystem = { pkgs, lib, ... }: {
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
    */
    config.writers = {
      writePureShellScript = availablePrograms: script:
        pkgs.writeScript "script.sh" ''
          #!${pkgs.bash}/bin/bash
          set -Eeuo pipefail

          export PATH="${lib.makeBinPath availablePrograms}"
          export NIX_PATH=nixpkgs=${pkgs.path}

          TMPDIR=$(${pkgs.coreutils}/bin/mktemp -d)

          trap '${pkgs.coreutils}/bin/chmod -R +w $TMPDIR;  ${pkgs.coreutils}/bin/rm -rf "$TMPDIR"' EXIT

          ${script}
        '';

      writePureShellScriptBin = binName: availablePrograms: script:
        pkgs.writeScriptBin binName ''
          #!${pkgs.bash}/bin/bash
          set -Eeuo pipefail

          export PATH="${lib.makeBinPath availablePrograms}"
          export NIX_PATH=nixpkgs=${pkgs.path}

          TMPDIR=$(${pkgs.coreutils}/bin/mktemp -d)

          trap '${pkgs.coreutils}/bin/chmod -R +w $TMPDIR;  ${pkgs.coreutils}/bin/rm -rf "$TMPDIR"' EXIT

          ${script}
        '';
    };
  };
}

