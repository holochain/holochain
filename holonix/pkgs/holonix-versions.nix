{ writeShellScriptBin }:

writeShellScriptBin "hn-versions" ''
  cat ${../VERSIONS.md}
''
