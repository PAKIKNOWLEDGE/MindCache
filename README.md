# MindCache

filesystem-first 个人知识库。AI Agent 是主要写入者，Markdown 文件是唯一数据源，
`mind` CLI 负责校验与生成静态 dashboard。

```
你说一句话 → Hermes（依据 skill/SKILL.md）→ mind new → ~/mind/
                              → mind check && mind build → ~/mind/dist/ → 浏览器
```

- 格式规范：[SPEC.md](SPEC.md)
- Agent 首次部署指南：[AGENTS.md](AGENTS.md)
- Agent 操作手册（即 Hermes 的 skill）：[skill/SKILL.md](skill/SKILL.md)

## 命令

| 命令 | 作用 |
| --- | --- |
| `mind init` | 初始化 vault（默认 `~/mind`，含 git init） |
| `mind new <type> [标题]` | 创建条目（thought / todo / idea / note） |
| `mind check` | lint 全部条目（文件名、frontmatter、type/目录一致性） |
| `mind build` | 生成静态站点到 `~/mind/dist/` |
| `mind serve --port 8181` | 局域网提供 dist 访问 |

vault 位置：`$MIND_VAULT` 或 `--vault PATH` 覆盖，默认 `~/mind`。

## NixOS 构建

```bash
nix-build -A mindcache          # 常规构建，产物 ./result/bin/mind
nix-build -A mindcache-musl     # 静态 musl 构建，单文件可拷贝到任意 Linux
nix-env -f . -iA mindcache      # 安装进用户 profile（PATH）
```

构建依赖 `Cargo.lock` 在仓库中（buildRustPackage 哈希校验需要）。

## 开发

```bash
cargo build --release
cargo run -- init /tmp/mind-dev
MIND_VAULT=/tmp/mind-dev cargo run -- check
```
