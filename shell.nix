{
  flavor ? "coreDev",
  ...
} @ args:
(import ./default.nix ({inherit flavor;} // args))
