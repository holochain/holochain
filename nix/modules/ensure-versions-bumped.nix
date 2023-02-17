{ self, lib, inputs, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }: let
    loadToml = file: builtins.fromTOML (builtins.readFile file);
    cargoToml = loadToml (self + /Cargo.toml);
    members = cargoToml.workspace.members;

    # script for a given member to fail if version is not bumped
    checkMember = member: let
      oldPath = lib.cleanSource (inputs.holochain + "/${member}");
      newPath = lib.cleanSource (self + "/${member}");
      oldVersion = (loadToml "${oldPath}/Cargo.toml").package.version;
      newVersion = (loadToml "${newPath}/Cargo.toml").package.version;
    in ''
      if [ "${oldPath}" != "${newPath}" ] && [ "${oldVersion}" == "${newVersion}" ]; then
        echo "ERROR: Crate ${member} has changed since the last release. Please bump its version."
        failed=true
      fi
    '';

    # script for all members to fail if at least one version bump is missing
    script = ''
      failed=false
      ${lib.concatStringsSep "\n" (map checkMember members)}
      if [ "$failed" == "true" ]; then
        exit 1
      fi
    '';
    program = config.writers.writePureShellScript [pkgs.coreutils] script;

  in {
    apps.ensure-versions-bumped.type = "app";
    apps.ensure-versions-bumped.program = toString program;
  };
}
