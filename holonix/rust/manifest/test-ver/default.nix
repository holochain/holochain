{ pkgs }:
let
  name = "hn-rust-manifest-test-ver";

  script = pkgs.writeShellScriptBin name ''
    # node dists can mess with the process
    hn-flush

    # loop over all tomls
    # find all possible upgrades
    # ignore upgrades that are just unpinning themselves (=x.y.z will suggest x.y.z)
    # | grep -vE 'v=([0-9]+\.[0-9]+\.[0-9]+) -> v\1'
    echo "attempting to suggest new pinnable crate versions"
    find . -name "Cargo.toml" | xargs -P "$NIX_BUILD_CORES" -I {} cargo upgrade --dry-run --allow-prerelease --all --manifest-path {} | grep -vE 'v=[0-9]+\.[0-9]+\.[0-9]+'

    hn-rust-manifest-list-unpinned
  '';
in { buildInputs = [ script ]; }
