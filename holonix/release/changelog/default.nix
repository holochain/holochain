{ pkgs, config }:
let
  name = "hn-release-changelog";

  heading-placeholder = "{{ version-heading }}";

  preamble = ''
    # Changelog
    The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
    This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
  '';

  template = ''
    ${preamble}
    ${heading-placeholder}

    ### Added

    ### Changed

    ### Deprecated

    ### Removed

    ### Fixed

    ### Security
  '';

  changelog-path = "./CHANGELOG.md";
  unreleased-path = "./CHANGELOG-UNRELEASED.md";

  script = pkgs.writeShellScriptBin name ''
    set -euo pipefail
    # locking off changelog version

    # ensure required files exist
    touch ${changelog-path}
    touch ${unreleased-path}

    template="$(cat ${unreleased-path})"
    heading_placeholder="${heading-placeholder}"
    heading="## [${config.release.version.current}] - $(date --iso --u)"
    prepend=''${template/$heading_placeholder/$heading}
    current=$( cat ./CHANGELOG.md | sed -e '1,4d' )
    echo "timestamping and retemplating changelog"
    printf '%s\n\n%s\n' "$prepend" "$current" > ${changelog-path}
    echo '${template}' > '${unreleased-path}'
  '';
in { buildInputs = [ script ]; }
