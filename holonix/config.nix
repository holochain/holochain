# empty config file full of fallbacks to get default.nix loading
# use this as a minimal template or example.config.nix for a full file
{
  release = {
    commit = "________________________________________";
    version = {
      current = "_._._";
      previous = "_._._";
    };

    hook = {
      preflight = ''
        echo "<your preflight script here>"
      '';
      version = ''
        echo "<your versioning script here>"
      '';
      publish = ''
        echo "<your publishing script here>"
      '';
    };

    github = {
      owner = "<your github owner here>";
      repo = "<your repo name here>";
      template = ''
        {{ changelog }}
        <your release template markdown here>
      '';
    };
  };

  holochain-nixpkgs = rec {
    use-github = true;

    # configuration for when use-github = false
    local = {
      # the path to the local holonix copy
      path = ../holochain-nixpkgs;
    };

    pathFn = _:
      if use-github then
        (import ./nix/sources.nix).holochain-nixpkgs
      else
        local.path;

    importFn = _: import (pathFn { }) { };
  };
}
