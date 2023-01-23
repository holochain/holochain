{ pkgs, config }:
let
  name = "hn-release-hook-version-readme";

  script = pkgs.writeShellScriptBin name ''
    echo "bumping versions from ${config.release.version.previous} to ${config.release.version.current} in readmes"
    find . \
     -iname "readme.md" \
     -not -path "**/.git/**" \
     -not -path "**/.cargo/**" | xargs -I {} \
     sed -i 's/${config.release.version.previous}/${config.release.version.current}/g' {}
  '';
in { buildInputs = [ script ]; }
