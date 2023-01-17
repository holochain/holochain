{ self, lib, inputs, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }: let

    hcMkShell = import ./hcMkShell.nix {
      inherit lib pkgs;
      inherit (self'.packages) nixEnvPrefixEval;
    };

  in {
    # shell for HC core development. included dependencies:
    # * everything needed to compile this repos' crates
    # * CI scripts
    devShells.coreDev = pkgs.callPackage ./coreDev.nix {
      inherit hcMkShell;
      inherit (config.rust.packages.holochain.rustPlatform.rust)
        cargo
        rustc
        ;
      inherit (config)
        coreScripts
        crate2nix
        ;
    };

    devShells.release = pkgs.callPackage ./release.nix {
      inherit (self') devShells;
      holochainSrc = self;
    };

    devShells.ci = pkgs.callPackage ./ci.nix {
      inherit hcMkShell;
      inherit (self') devShells;
      inherit (self'.packages)
        ciSetupNixConf
        ciCachixPush
        ;
    };

    devShells.happDev = pkgs.callPackage ./happDev.nix {
      inherit hcMkShell;
      inherit (self') devShells;
      inherit (self'.packages)
        ciSetupNixConf
        ciCachixPush
        happ-holochain
        happ-hc
        ;
    };

    devShells.coreDevRustup = self'.devShells.coreDev;
  };
}

