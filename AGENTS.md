# AGENTS.md — MindCache 开发对接

本文档给**开发/修改 MindCache 源码**的 Agent 对接开发进度与工程约定。

- 给"在目标机器部署/更新 mind 二进制"的 Agent → `DEPLOY.md`
- vault 的数据读写（捕获/检索/整理/提交规范）→ `skill/SKILL.md`
- 数据格式唯一权威 → `SPEC.md`（一切读写先读它）

## 项目速览

- 单文件 Rust CLI（`src/main.rs`，仅 chrono/comrak 依赖），子命令分派；数据层 = Markdown + YAML frontmatter + 目录结构，文件是唯一 source of truth。
- `mind build` 产出 `dist/` 静态 dashboard：index + 4 分类页 + `tags.html` 聚合页 + `search.json`（面板检索索引）+ 每条目详情页。
- 数据完整性禁令（时间戳前缀、frontmatter 字段名、`archive/` 不删）在 SPEC §4，**改行为先改 SPEC 再改代码**。

## 开发环境（本机无 rustc，统一走 nix develop）

```bash
nix develop -f shell.nix                          # rustc 1.95 / cargo / cc；crates.io 已走国内镜像（env 内）
nix develop -f shell.nix -c cargo build           # 编译/校验
# 产物 ./target/debug/mind 是普通 ELF，可直接在 shell 外运行
```

不要在本机装独立 rustc；不要往仓库提交 `.cargo/config.toml`（镜像配置只存在于 `shell.nix` env，避免影响 nix-env 构建）。

## 验证约定（防污染）

- 一律 `MIND_VAULT=/tmp/<沙箱> --vault` 指向临时目录；**禁止裸 `mind init`**（会写用户真实 `~/.config/mind/config.toml`）。
- 命令行行为：fixture 搭在 `/tmp` 沙箱内，逐命令核对输出与退出码。
- dashboard 交互（折叠/搜索/tag 导航）：用 **node + jsdom 跑页面内联 JS**（装到 /tmp 目录；本环境 eval 沙箱拉不起 Chromium）。断言覆盖：折叠行数/展开、搜索命中计数与链接 textContent、tag chip↔锚点、file:// fetch 失败降级。
- 交付前核对：真实 `~/.config/mind`、`~/mind`、`~/.cargo` 零改动。

## 当前进度（2026-09-09）

**已完成**：`mind search` CLI + dashboard 搜索面板（search.json 索引）；tags 聚合页 + 可点击 tag（percent-encoded 锚点）；check 目录/type 不一致降级 WARN（SPEC §1 已同步）；`mind new --body/--tags/stdin`；`mind done/reopen/archive` 状态机；`serve --bind`；RECENT 8 条折叠 + todo done 折叠；parse_fm 成对引号/flow tags 修复。

**已知取舍/待办**：

- tag 不做归一（大小写、全半角视为不同 tag）——自由标签哲学，接受。
- `mind search` 为线性扫描，超大 vault（数千文件）会慢——可接受，不做索引缓存。
- 未做：archive 独立页、详情页邻接导航、TODO 面板 12 条以上折叠改用 foldwrap 按钮（当前仍是"… more open todos"链接）。
- 规则变更时同步面：`usage()`、README 命令表、`skill/SKILL.md`、`DEPLOY.md`。

## 变更纪律

- 改命令/输出/校验规则 → 同步 `usage()`、README 命令表、`skill/SKILL.md`、`DEPLOY.md` 四处文档。
- 改 SPEC 行为（字段/禁令/状态机）→ 先改 `SPEC.md` 再改代码。
- **提交信息写详细**：首行 ≤50 字概括（动词开头），空行后列改动要点与原因，多 `-m` 或 heredoc；禁止"update files"式空话。仓库提交与 vault 提交同规范（vault 侧见 SKILL.md 维护段）。