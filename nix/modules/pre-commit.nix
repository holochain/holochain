{ self, lib, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }:
    let
      check-workflow-docs-script = config.writers.writePureShellScript
        (with pkgs; [
          coreutils
        ])
        ''
          for yaml in ./.github/{actions/*,workflows}/*.y*ml; do
            name=''${yaml%.*}
            if [ ! -e "$name.md" ]; then
              >&2 echo "ERROR: Missing documentation for workflow $yaml. Please create $name.md"
              exit 1
            fi
          done
        '';
    in
    {
      pre-commit.check.enable = true;
      pre-commit.settings.hooks.nixpkgs-fmt.enable = true;
      pre-commit.settings.hooks.check-workflow-docs = {
        enable = true;
        description = "Ensures that documentation exists for each github workflow file";
        entry = "${check-workflow-docs-script}";
      };
    };
}
