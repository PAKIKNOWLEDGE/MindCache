{ pkgs ? import <nixpkgs> {} }:

# 开发用环境：本机无 rustc 工具链，编译/check 一律经 nix-shell 进入本环境。
#    nix-shell --run "cargo build"          # 编译
#    nix-shell --run "cargo check"          # 快速语法检查
# 产物 ./target/debug/mind 是普通 ELF，构建完成后可直接在 shell 外运行。

pkgs.mkShell {
  buildInputs = with pkgs; [
    rustc
    cargo
    gcc # rustc 链接需要 C 链接器（纯 Rust 依赖，不用 pkg-config）
  ];

  # crates.io 走国内镜像（仅本开发环境生效；不影响用户全局 cargo，
  # 也不影响 nix buildRustPackage 的 vendored 依赖下载）
  env = {
    CARGO_REGISTRIES_CRATES_IO_REPLACE_WITH = "tuna";
    CARGO_REGISTRIES_TUNA_REGISTRY = "sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/";
  };
}