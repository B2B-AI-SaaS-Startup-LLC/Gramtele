{ pkgs ? import (let
    lock = builtins.fromJSON (builtins.readFile ./flake.lock);
    pinned = lock.nodes.nixpkgs.locked;
  in builtins.fetchTarball {
    url = "https://github.com/NixOS/nixpkgs/archive/${pinned.rev}.tar.gz";
    sha256 = pinned.narHash;
  }) { } }:
pkgs.mkShell {
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [ pkgs.libxcb pkgs.libxcb-keysyms ];
  packages = with pkgs; [ minizip cargo rustc clippy rustfmt gcc pkg-config dbus xorg-server libxcb libxcb-keysyms ];
}
