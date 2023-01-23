{ pkgs, holochainVersionId, holochainVersion }:

let
  extraSubstitutors = [ "https://cache.holo.host" ];
  trustedPublicKeys = [
    "cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspcDShjY="
    "cache.holo.host-1:lNXIXtJgS9Iuw4Cu6X0HINLu9sTfcjEntnrgwMQIMcE="
    "cache.holo.host-2:ZJCkX3AUYZ8soxTLfTb60g+F3MkWD7hkH9y8CgqwhDQ="
  ];

  buildCmd = ''
    $(command -v nix-store) \
        --option extra-substituters "${
          builtins.concatStringsSep " " extraSubstitutors
        }" \
        --option trusted-public-keys  "${
          builtins.concatStringsSep " " trustedPublicKeys
        }" \
        --add-root "''${GC_ROOT_DIR}/allrefs" --indirect \
        --realise "''${ref}"
  '';

in pkgs.writeShellScriptBin "holonix" ''
  export GC_ROOT_DIR="''${HOME:-/tmp}/.holonix"
  export SHELL_DRV="''${GC_ROOT_DIR}/shellDrv"
  export LOG="''${GC_ROOT_DIR}/log"

  cat <<- EOF
  # Holonix

  ## Permissions
  This scripts uses sudo to allow specifying Holo's Nix binary cache. Specifically:
  * Instruct Nix to use the following extra substitutors (binary cache):
    - ${builtins.concatStringsSep "\n - " extraSubstitutors}
  * Instruct Nix to use trust these public keys:
    - ${builtins.concatStringsSep "\n  - " trustedPublicKeys}

  If you don't want to use "sudo", you can set HN_NOSUDO="true" prior to calling this script.

  ## Caching
  Holonix will be cached locally.
  To wipe the cache, remove all symlinks inside ''${GC_ROOT_DIR} and run "nix-collect-garbage".

  ## Running the cached version directly
  Use: nix-shell ''${SHELL_DRV}

  Building...
  EOF

  if [[ $(uname) == "Darwin" ]]; then
    echo macOS detected, disabling sudo.
    export HN_NOSUDO="true"
  fi

  set -euo pipefail
  mkdir -p "''${GC_ROOT_DIR}"

  function handle_error() {
    rc=$?

    echo "Errors during build. Status: $rc)"
    if [[ -e ''${SHELL_DRV} ]]; then
        echo Please see "''${LOG}" for details.
        echo Falling back to cached version
    else
        cat ''${LOG}
        exit $rc
    fi
  }
  trap handle_error err

  function handle_int() {
    rc=$?
    if [[ ''${HN_VERBOSE:-false} != "true" ]]; then
      echo Check ''${LOG} for the build output.
    fi
    echo Aborting.
    exit $rc
  }
  trap handle_int int

  (
    if [[ ''${HN_VERBOSE:-false} != "true" ]]; then
      exec 2>''${LOG} 1>>''${LOG}
    fi

    SHELL_DRV_TMP=$(mktemp)
    rm ''${SHELL_DRV_TMP}

    nix-instantiate --add-root "''${SHELL_DRV_TMP}" --indirect ${
      builtins.toString ./.
    }/.. -A main \
      --argstr holochainVersionId ${holochainVersionId} \
      --arg holochainVersion '{ rev = "${holochainVersion.rev}"; sha256 = "${holochainVersion.sha256}"; cargoSha256 = "${holochainVersion.cargoSha256}"; }'
    for ref in `nix-store \
                --add-root "''${GC_ROOT_DIR}/refquery" --indirect \
                --query --references "''${SHELL_DRV_TMP}"`;
    do
      echo Processing ''${ref}

      if [[ "''${HN_NOSUDO:-false}" == "true" ]]; then
        ${buildCmd}
      else
        sudo -E ${buildCmd}
      fi
    done

    mv -f ''${SHELL_DRV_TMP} ''${SHELL_DRV}
  )

  echo Spawning shell..
  export NIX_BUILD_SHELL="${pkgs.runtimeShell}"
  nix-shell \
    --add-root "''${GC_ROOT_DIR}/finalShell" --indirect \
    "''${SHELL_DRV}" "''${@}"
''
