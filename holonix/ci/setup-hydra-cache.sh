#!/usr/bin/env sh
set -xe
mkdir -p ~/.config/nix/
cat > ~/.config/nix/nix.conf << 'EOF'
substituters = https://cache.nixos.org/ https://cache.holo.host/
trusted-public-keys = cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspcDShjY= cache.holo.host-1:lNXIXtJgS9Iuw4Cu6X0HINLu9sTfcjEntnrgwMQIMcE= cache.holo.host-2:ZJCkX3AUYZ8soxTLfTb60g+F3MkWD7hkH9y8CgqwhDQ=
EOF
