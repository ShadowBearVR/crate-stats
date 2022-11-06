{ pkgs ? import <nixpkgs> {} }:
with pkgs; mkShell {
  name = "crate-stats";
  
  shellHook =
  ''
    export RUSTUP_TOOLCHAIN="nightly"
    export OPENSSL_DIR="${pkgs.openssl}"
    export OPENSSL_INCLUDE_DIR="${pkgs.openssl.dev}/include"
    export OPENSSL_LIB_DIR="${pkgs.openssl.out}/lib"
  '';

  packages = [
    rustup
    binutils
    openssl
    cmake
    gcc
    libgit2
 ];
}