{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/release-25.05";

  inputs.flake-utils.url = "github:numtide/flake-utils";

  # ( printf "~20024642E00202FD33\r"; sleep 1 ) | nc 192.168.10.237 23
  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            config.allowUnfree = true;
          };
        in
        {
          devShells.default = pkgs.mkShell {
            nativeBuildInputs = with pkgs.buildPackages; [
              git
              socat
              ser2net

              (python313.withPackages (python-pkgs: [
                python-pkgs.pyserial
                python-pkgs.construct
                python-pkgs.standard-telnetlib
              ]))
            ];
          };
        }
      );
}
