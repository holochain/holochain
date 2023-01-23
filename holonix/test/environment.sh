#! /usr/bin/env nix-shell
#! nix-shell -i "bats -t" -p bats -p coreutils

@test "environment variables are set properly" {

 standard_nix_env_prefix=$( nix-shell --run 'echo $NIX_ENV_PREFIX' )
 [[ $standard_nix_env_prefix == $PWD ]]

 standard_cargo_home=$( nix-shell --run 'echo $CARGO_HOME' )
 [[ $standard_cargo_home == "$PWD/.cargo" ]]

 standard_cargo_install_root=$( nix-shell --run 'echo $CARGO_INSTALL_ROOT' )
 [[ $standard_cargo_install_root == "$PWD/.cargo" ]]

 standard_path=$( nix-shell --run 'echo $PATH' )
 [[ $standard_path == $PWD/.cargo/bin:* ]]

 override_nix_env_prefix=$( NIX_ENV_PREFIX=foo nix-shell --run 'echo $NIX_ENV_PREFIX' )
 [[ $override_nix_env_prefix == 'foo' ]]

 override_cargo_home=$( NIX_ENV_PREFIX=foo nix-shell --run 'echo $CARGO_HOME' )
 [[ $override_cargo_home = 'foo/.cargo' ]]

 override_cargo_install_root=$( NIX_ENV_PREFIX=foo nix-shell --run 'echo $CARGO_INSTALL_ROOT' )
 [[ $override_cargo_install_root == 'foo/.cargo' ]]

 override_path=$( NIX_ENV_PREFIX=foo nix-shell --run 'echo $PATH' )
 [[ $override_path == foo/.cargo/bin:* ]]

}
