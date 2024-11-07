{
    description = "A Nix-flake-based Rust development environment";

    inputs = {
        nixpkgs.url = "https://flakehub.com/f/NixOS/nixpkgs/*.tar.gz";
        rust-overlay = {
            url = "github:oxalica/rust-overlay";
            inputs.nixpkgs.follows = "nixpkgs";
        };
        parts = {
            url = "github:hercules-ci/flake-parts";
            inputs.nixpkgs-lib.follows = "nixpkgs";
        };
        nix-filter.url = "github:numtide/nix-filter";
    };

#            overlays = [
#                rust-overlay.overlays.default
#                (final: prev: {
#                    rustToolchain =
#                        let
#                            rust = prev.rust-bin;
#                        in
#                        if builtins.pathExists ./rust-toolchain.toml then
#                            rust.fromRustupToolchainFile ./rust-toolchain.toml
#                        else if builtins.pathExists ./rust-toolchain then
#                            rust.fromRustupToolchainFile ./rust-toolchain
#                        else
#                            rust.stable.latest.default.override {
#                                extensions = [ "rust-src" "rustfmt" ];
#                            };
#                })
#            ];


    outputs = inputs@{ self, nixpkgs, rust-overlay, parts, nix-filter }:
    parts.lib.mkFlake { inherit inputs; } {
        systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
        perSystem = { self', lib, system, ... }: 
        let 
            pkgs = nixpkgs.legacyPackages.${system};
            cargo_toml = builtins.fromTOML( builtins.readFile ./Cargo.toml);
            dev_tools = with pkgs; [
                pkg-config
                cargo-deny
                cargo-edit
                cargo-watch
                rust-analyzer
                rustfmt
            ];
        in
        {
            packages.default = pkgs.rustPlatform.buildRustPackage {
                pname = cargo_toml.package.name;
                version = cargo_toml.package.version;
                src = nix-filter.lib.filter {
                    root = ./.;
                    include = [
                        ./src
                        ./Cargo.toml
                        ./Cargo.lock
                        ./rust-toolchain.toml
                        ./foo
                    ];
                };

                cargoLock = {
                    lockFile = ./Cargo.lock;
                    allowBuiltinFetchGit = lib.mkDefault false;
                };
            };
            devShells.default = pkgs.mkShell {
                inputsFrom = [ self'.packages.default ];
                packages = dev_tools;
            };
        };
    };
}
