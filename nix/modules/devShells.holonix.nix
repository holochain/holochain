{ inputs, self, lib, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }:
    let

      holonix' = pkgs.mkShell {
        inputsFrom = [ self'.devShells.rustDev ];
        packages = holonixPackages ++ [ hn-introspect ];
        shellHook = ''
          echo Holochain development shell spawned. Type 'exit' to leave.
          export PS1='\n\[\033[1;34m\][holonix:\w]\$\[\033[0m\] '
        '';
      };
      holonixPackages = with self'.packages; [ holochain lair-keystore hc-launch hc-scaffold ];
      versionsFileText = builtins.concatStringsSep "\n"
        (
          builtins.map
            (package: ''
              echo ${package.pname} \($(${package}/bin/${package.pname} -V)\): ${package.src.rev or "na"}'')
            holonixPackages
        );
      hn-introspect =
        pkgs.writeShellScriptBin "hn-introspect" versionsFileText;

      versionsInputSpecified = (builtins.pathExists "${inputs.versions.outPath}/flake.nix") || (builtins.readFile inputs.versions.outPath != "");
      holonix =
        if versionsInputSpecified
        then
          builtins.trace
            ''
              DEPRECATION WARNING: 'inputs.versions` has been specified (timestamp: ${toString inputs.versions.lastModified}).
                  
              it has been deprecated due to unintended behavior.

              the approach that is known to work better is to define the versions flake as a distinct input in your project's flake. 
              then configure the holochain-flake to follow all versions by defining individual follows.

              consider the following example flake.nix:

              ${builtins.readFile (self + /holonix/examples/custom_versions/flake.nix)}
            ''
            holonix'
        else holonix';
    in
    {
      devShells.holonix = holonix;

      packages = {
        inherit hn-introspect;
      };
    };
}
