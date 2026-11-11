{
  description = "remote-input development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { nixpkgs, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            go
            gopls
            delve
            git
            ripgrep
            nodejs
            pnpm
          ] ++ lib.optionals pkgs.stdenv.hostPlatform.isLinux [
            xclip
            wl-clipboard
          ];
          CGO_ENABLED = "0";
          GOTOOLCHAIN = "local";
        };
      });
    };
}
