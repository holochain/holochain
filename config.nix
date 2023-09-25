{

  # configure holonix itself
  holonix = rec {

    # true = use a github repository as the holonix base (recommended)
    # false = use a local copy of holonix (useful for debugging)
    use-github = true;

    includeHolochainBinaries = false;
    includeScaffolding = false;
    includeTest = false;
    includeNode = false;
    includeDocs = false;
    includeHapps = false;
    includeRelease = false;

    importFn = args: import ./holonix (args // ({
      include = (args.include or { }) // {
        scaffolding = args.include.scaffolding or includeScaffolding;
        test = args.include.test or includeTest;
        holochainBinaries = args.include.holochainBinaries or includeHolochainBinaries;

        node = args.include.node or includeScaffolding;
        docs = args.include.docs or includeDocs;
        happs = args.include.happs or includeHapps;
        release = args.include.release or includeRelease;
      };
    }));
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
