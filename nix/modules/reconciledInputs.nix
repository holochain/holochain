{ self, lib, inputs, ... }:

{
  options.reconciledInputs = lib.mkOption { type = lib.types.raw; };
  config.reconciledInputs = lib.genAttrs (builtins.attrNames inputs.versions.inputs)
    (name:
      let
        input =
          if builtins.pathExists (inputs."${name}" + "/Cargo.toml")
          then inputs."${name}"
          else
            inputs.versions.inputs."${name}";
        rev = input.rev or self.rev;
      in
      (input // { inherit rev; })
    );
}
