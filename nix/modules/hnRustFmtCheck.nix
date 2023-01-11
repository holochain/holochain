{ self, lib, ... }: {
  perSystem = { config, self', inputs', pkgs, ... }: {
    packages.hnRustFmtCheck = pkgs.writeShellScriptBin "hn-rust-fmt-check" ''
      echo "checking rust formatting"
      cargo fmt -- --check
    '';
  };
}
