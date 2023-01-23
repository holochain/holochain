{ lib, pkgs, writeShellScriptBin, holochainVersionId, holochainBinaries
, pkgsOfInterest ?
  lib.optionalAttrs pkgs.stdenv.isLinux { inherit (pkgs.linuxPackages) perf; }
, cmdsOfInterest ? [ "rustc" "cargo fmt" "cargo clippy" ], binaryNameMapping ? {
  launcher = "hc-launch";
  scaffolding = "hc-scaffold";
} }:

let
  namesVersionsStringPkgs = { packages, run }:
    lib.attrsets.mapAttrsToList (name: value:
      if !builtins.isAttrs value then
        ""
      else
        ("echo - ${name}" + (if builtins.hasAttr "version" value then
          "-${value.version}"
        else if run == true then
          "-$(${
            binaryNameMapping."${name}" or name
          } --version | cut -d' ' -f2-)"
        else
          "") + (if !builtins.hasAttr "src" value then
            ""
          else
            let
              url = builtins.toString (value.src.urls or value.src.url or "");
              prefix = if url == "" then "" else ": ";
              rev =
                if url == "" then "" else builtins.toString value.src.rev or "";
              delim = if rev == "" then
                ""
              else if lib.strings.hasInfix "github.com" url then
                "/tree/"
              else
                "#";
            in prefix + url + delim + rev))) packages;

  namesVersionsStringBins = cmds:
    builtins.map (bin:
      "echo - ${bin}: $(${bin} --version)"
      # else "$(${name} --version | ${gawk}/bin/awk '{ print $2}')"
    ) cmds;

in writeShellScriptBin "hn-introspect" ''
  function hcInfo() {
    echo ${holochainVersionId}
    ${
      builtins.concatStringsSep "\n" (namesVersionsStringPkgs {
        packages = holochainBinaries;
        run = true;
      })
    }
  }

  function commonInfo() {
    ${builtins.concatStringsSep "\n" (namesVersionsStringBins cmdsOfInterest)}
    ${
      builtins.concatStringsSep "\n" (namesVersionsStringPkgs {
        packages = pkgsOfInterest;
        run = false;
      })
    }
  }

  case "$1" in
    "hc")
      hcInfo
      ;;

    "common")
      commonInfo
      ;;

    *)
      echo List of applications and their version information
      echo
      hcInfo
      echo ""
      commonInfo
  esac
''
