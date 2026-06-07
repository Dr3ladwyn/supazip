{
  description = "SupaZip — cross-platform archive manager";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in {
        packages.supazip = pkgs.rustPlatform.buildRustPackage {
          pname = "supazip";
          version = "1.0.0";
          src = ./.;
          cargoLock.lockFile = ./supazip/Cargo.lock;
          buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.Security
          ];
          meta = with pkgs.lib; {
            description = "Cross-platform archive manager for 7z and ZIP";
            license = with licenses; [ mit asl20 ];
            maintainers = [ ];
          };
        };
        defaultPackage = self.packages.${system}.supazip;
      }
    );
}
