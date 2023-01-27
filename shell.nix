{ devShellId ? "coreDev", ... }@args:
(import ./default.nix ({ inherit devShellId; } // args))
