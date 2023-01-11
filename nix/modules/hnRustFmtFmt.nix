{ self, lib, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }: {
    packages.hnRustFmtFmt = pkgs.writeShellScriptBin "hn-rust-fmt" ''
      cargo fmt
    '';
  };
}
