{ pkgs, config }:
let
  name = "hn-release-hook-publish-crates-io";

  script = pkgs.writeShellScriptBin name ''
    set -euox pipefail
    echo "packaging for crates.io"
    # @TODO - ship remove-dev-dependencies to crates io so we can use it everywhere
    # cargo run --manifest-path crates/remove-dev-dependencies/Cargo.toml crates/**/Cargo.toml
    # order is important here due to dependencies
    for crate in ''${1}
    do
     cargo publish --manifest-path "crates/$crate/Cargo.toml" --allow-dirty
     # need to wait 'long enough' for crates.io to finish processing the previous
     # crate so that subsequent ones can see it
     # there is no specific amount of time that crates.io guarantees but it is
     # usually only a few seconds (have observed timeouts at 10 seconds)
     # there is a registry API that can be hit to try and detect when the crate is
     # ready but unfortunately it returns 'OK' for a crate before cargo can find the
     # crate for subsequent publishing, so it is an unreliable test for our needs
     sleep 30
    done
    git checkout -f
  '';
in { buildInputs = [ script ]; }
