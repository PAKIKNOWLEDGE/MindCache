{ pkgs ? import <nixpkgs> {} }:

let
  version = "0.1.0";
  src = ./.;

  # 常规动态构建（二进制名是 mind）
  mindcache = pkgs.rustPlatform.buildRustPackage {
    pname = "mindcache";
    inherit version src;

    cargoLock.lockFile = ./Cargo.lock;

    meta = with pkgs.lib; {
      description = "MindCache — filesystem-first personal knowledge base CLI (command: mind)";
      license = licenses.mit;
      maintainers = [];
      platforms = platforms.linux;
    };
  };

  # 静态 musl 构建（二进制可 scp 到任意 Linux 机器直接跑）
  mindcache-musl = pkgs.pkgsCross.musl64.rustPlatform.buildRustPackage {
    pname = "mindcache";
    inherit version src;

    cargoLock.lockFile = ./Cargo.lock;

    RUSTFLAGS = "-C target-feature=+crt-static";

    meta = with pkgs.lib; {
      description = "MindCache CLI (static musl build, command: mind)";
      license = licenses.mit;
      maintainers = [];
      platforms = [ "x86_64-linux" ];
    };
  };
in
{
  inherit mindcache mindcache-musl;
}
