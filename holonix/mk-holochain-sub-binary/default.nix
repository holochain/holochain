{ sources, name, holochainBinaries, holochainVersionFinal }:

{
  buildInputs = if (holochainVersionFinal."${name}" or null) == null then
    builtins.trace "WARNING binary was ${name} requested but not found in ${
      builtins.toJSON holochainVersionFinal
    }" [ ]
  else
    [ holochainBinaries."${name}" ];
}
