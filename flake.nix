{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/release-25.05";

  inputs.flake-utils.url = "github:numtide/flake-utils";

  inputs.pyproject-nix = {
    url = "github:pyproject-nix/pyproject.nix";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  inputs.uv2nix = {
    url = "github:pyproject-nix/uv2nix";
    inputs.pyproject-nix.follows = "pyproject-nix";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  inputs.pyproject-build-systems = {
    url = "github:pyproject-nix/build-system-pkgs";
    inputs.pyproject-nix.follows = "pyproject-nix";
    inputs.uv2nix.follows = "uv2nix";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  # ( printf "~20024642E00202FD33\r"; sleep 1 ) | nc 192.168.10.237 23
  outputs =
    { self
    , nixpkgs
    , flake-utils
    , uv2nix
    , pyproject-nix
    , pyproject-build-systems
    , ...
    }:
    flake-utils.lib.eachDefaultSystem
      (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          config.allowUnfree = true;
        };
        lib = pkgs.lib;
     workspace = uv2nix.lib.workspace.loadWorkspace { workspaceRoot = ./.; };

      overlay = workspace.mkPyprojectOverlay {
        sourcePreference = "wheel"; # or sourcePreference = "sdist";
      };

      pyprojectOverrides = _final: _prev: {
      };

      python = pkgs.python313;

      pythonSet =
        (pkgs.callPackage pyproject-nix.build.packages {
          inherit python;
        }).overrideScope
          (
            lib.composeManyExtensions [
              pyproject-build-systems.overlays.default
              overlay
              pyprojectOverrides
            ]
          );
      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs.buildPackages; [
            git
            socat
            uv

            (python313.withPackages (python-pkgs: [
              python-pkgs.pyserial
              python-pkgs.construct
              python-pkgs.standard-telnetlib
              python-pkgs.rich
            ]))
          ];
        };

     packages.default = pythonSet.mkVirtualEnv "pylontechpoller-env" workspace.deps.default;

        apps.default = {
          type = "app";
          program = "${self.packages."${system}".default}/bin/poller";
        };
      }
      );


}
