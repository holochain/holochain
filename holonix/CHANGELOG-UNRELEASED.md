# Changelog
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

{{ version-heading }}

### Added

* Introduced the _[VERSIONS.md](./VERSIONS.md)_ file which is regenerated when updating nix sources are updated via the _nix/update.sh_ script.
  It lists all pre-built _holochainVersionId_s with their version information, as well as common binaries and their version.

### Changed

* Tracking specific holochain versions that can be selected via the _holochainVersionId_ argument. Please see _[VERSIONS.md](./VERSIONS.md)_ for all existing values and their corresponding holochain version.

* ***Breaking changes*** to _holochainVersion_.

  With [a change in holonchain-nixpkgs](https://github.com/holochain/holochain-nixpkgs/pull/17) the attributes "cargoSha256" "bins" "lairKeystoreHashes" are no longer supported by _holochainVersion_.
  If you rely on using a custom Holochain version that is not pre-built please take a look at [the provided example](./examples/custom-holochain/).

### Removed

* Removed the _docs_ component as the site has been obsoleted long ago.
