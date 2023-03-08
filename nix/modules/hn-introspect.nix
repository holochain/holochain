{ self, lib, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }:
    let

      holonixPackages = with self'.packages;
        [ holochain lair-keystore hc-launch hc-scaffold ];

      inspectVersionFor = pkg:
        ''echo ${pkg.pname} \($(${pkg}/bin/${pkg.pname} -V)\): ${pkg.src.rev or "na"}'';

      versionsFileText = builtins.concatStringsSep "\n"
        (builtins.map inspectVersionFor holonixPackages);

    in
    {
      packages.hn-introspect = pkgs.writeShellScriptBin "hn-introspect" versionsFileText;
    };
}
