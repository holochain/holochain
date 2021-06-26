{

 # configure holonix itself
 holonix = rec {

  # true = use a github repository as the holonix base (recommended)
  # false = use a local copy of holonix (useful for debugging)
  use-github = true;

  # controls whether holonix' holochain binaries (holochain, hc, etc.) are included in PATH
  includeHolochainBinaries = false;

  # configure the remote holonix github when use-github = true
  github = {

   # can be any github ref
   # branch, tag, commit, etc.
   ref = "744d2d0b87eae44c56e5ec59918979f8709f30fe";

   # the sha of what is downloaded from the above ref
   # note: even if you change the above ref it will not be redownloaded until
   #       the sha here changes (the sha is the cache key for downloads)
   # note: to get a new sha, get nix to try and download a bad sha
   #       it will complain and tell you the right sha
   sha256 = "10imi41v1q03y6j6q90dhv4hk775nb2gzxjs8zr0ichbw3p5dwim";

   # the github owner of the holonix repo
   owner = "holochain";

   # the name of the holonix repo
   repo = "holonix";
  };

  # configuration for when use-github = false
  local = {
   # the path to the local holonix copy
   path = ../holonix;
  };

  pathFn = _: if use-github
     then builtins.fetchTarball (with github; {
        url = "https://github.com/${owner}/${repo}/archive/${ref}.tar.gz";
        inherit sha256; }
       )
     else local.path;

  importFn = _: import (pathFn {}) {
      inherit includeHolochainBinaries;
    }
    ;
 };

 release = {
  hook = {
   # sanity checks before deploying
   # to stop the release
   # exit 1
   preflight = ''
hn-release-hook-preflight-manual
'';

   # bump versions in the repo
   version = ''
hn-release-hook-version-rust
hcp-release-hook-version
'';

   # publish artifacts to the world
   publish = ''
# crates are published from circle!
'';
  };

  # the commit hash that the release process should target
  # this will always be behind what ends up being deployed
  # the release process needs to add some commits for changelog etc.
  commit = "8fb82a3a6d8cc69c95c654bd21bf15785a6ca291";

  # the semver for prev and current releases
  # the previous version will be scanned/bumped by release scripts
  # the current version is what the release scripts bump *to*
  version = {
   current = "0.0.13";
   # not used by version hooks in this repo
   previous = "_._._";
  };

  github = {
   # markdown to inject into github releases
   # there is some basic string substitution {{ xxx }}
   # - {{ changelog }} will inject the changelog as at the target commit
   template = ''
{{ changelog }}

# Installation

Use Holonix to work with this repository.

See:

- https://github.com/holochain/holonix
- https://nixos.org/
'';

   # owner of the github repository that release are deployed to
   owner = "holochain";

   # repository name on github that release are deployed to
   repo = "holochain";

   # canonical local upstream name as per `git remote -v`
   upstream = "origin";
  };
 };
}
