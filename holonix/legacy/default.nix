let
  holonixPath = ../.; # points to the current state of the Holochain repository
  holonix = import holonixPath {
    holochainVersionId = "v0_1_0"; # specifies the Holochain version
  };
  nixpkgs = holonix.pkgs;
in
nixpkgs.mkShell {
  inputsFrom = [ holonix.main ];
  packages = with nixpkgs; [
    niv
    nodejs-18_x
    # any additional packages needed for this project, e. g. Nodejs
  ];
}
