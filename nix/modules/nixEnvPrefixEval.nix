{ self, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }: {
    # Definitions like this are entirely equivalent to the ones
    # you may have directly in flake.nix.
    packages.nixEnvPrefixEval = pkgs.writeScript "nix-env-prefix-eval" ''
      if [[ -n "$NIX_ENV_PREFIX" ]]; then
        # don't touch it
        :
      elif test -w "$PWD"; then
        export NIX_ENV_PREFIX="$PWD"
      elif test -d "${self}" &&
          test -w "${self}"; then
        export NIX_ENV_PREFIX="${self}"
      elif test -d "$HOME" && test -w "$HOME"; then
        export NIX_ENV_PREFIX="$HOME/.cache/holochain-dev"
        mkdir -p "$NIX_ENV_PREFIX"
      else
        export NIX_ENV_PREFIX="$(${pkgs.coreutils}/bin/mktemp -d)"
      fi
    '';
  };
}
