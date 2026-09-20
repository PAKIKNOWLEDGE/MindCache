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
- dashboard 交互：`node tests/dashboard.mjs <dist>`（**node + jsdom** 跑页面内联 JS；jsdom 装到 /tmp，用 `NODE_PATH` 指过去）。断言覆盖：折叠行数/展开、搜索命中计数与链接 textContent、tag chip↔锚点、无 fetch 降级、外观/布局持久化与旧键迁移、单 h1、地标、站内链接全部落地。
- dashboard 视觉：`.grok/skills/ark-ui/scripts/audit-ark-ui.mjs`（对 `assets/ark/` 与一份构建产物副本各跑一次）；对比度/悬停/焦点/目标尺寸/`prefers-reduced-motion`/溢出用临时目录里的无头 Chromium 脚本量测（脚本不进仓库，属于一次性核查）。
- 交付前核对：真实 `~/.config/mind`、`~/mind`、`~/.cargo` 零改动。

## 当前进度（2026-09-20）

**已完成**：`mind search` CLI + dashboard 搜索面板（search.json 索引）；tags 聚合页 + 可点击 tag（percent-encoded 锚点）；check 目录/type 不一致降级 WARN（SPEC §1 已同步）；`mind new --body/--tags/stdin`；`mind done/reopen/archive` 状态机；`serve --bind`；RECENT 8 条折叠 + todo done 折叠；parse_fm 成对引号/flow tags 修复。

**Dashboard 视觉重写（按本地 skill `ark-ui` 的契约）**：

- 契约锁定：family `exa`（固定）、depth `moderate`、外观轴 auto/light/dark；根属性 `data-ark-theme` / `data-ark-depth` / `data-ark-appearance` / `data-ark-layout`。
- 样式从 `concat!` 字符串常量搬到 `assets/ark/`（appearance / family-exa / depth-moderate / base），构建时 `include_str!` 拼成 `dist/style.css`；Rust 侧拆出 `src/render.rs`（页面与组件渲染），`src/main.rs` 只留 CLI、数据层与文件写入。
- shell 重组：左侧竖栏（窄屏降级为底部动作条）、页面级舞台（标识符 + 库内分布圆环 + 14 天捕获条）、左缘年表刻度（按 created 等比落位）、`header/main/footer/nav` 地标、每页单个 `h1`。
- 删掉了旧 endfield 皮肤的幽灵大字与全屏加载动画（skill：装饰必须有信息作用、主内容不得藏在 splash 后）；旧键 `mind-theme`/`mind-density` 一次性迁移到新键。
- 上一版的已知缺陷逐条修掉：面板裁切（去掉文字容器上的 `overflow:hidden`）、悬停反白导致的低对比度（改为规则线/下划线 + 文字色不变）、窄屏横向溢出（`minmax(0,1fr)`）、tags 页 N 个盒子改为 chip 索引 + 规则带。
- 新增 `tests/dashboard.mjs`（36 项断言）；`audit-ark-ui.mjs` 对 CSS 与构建产物 0 error / 0 warning。

**已知取舍/待办**：

- tag 不做归一（大小写、全半角视为不同 tag）——自由标签哲学，接受。
- `mind search` 为线性扫描，超大 vault（数千文件）会慢——可接受，不做索引缓存。
- 未做：archive 独立页（archive 条目现在只在 tags 页与详情页可见）；行内 tag 链接的命中区靠纵向 padding 放大（WCAG 内联目标豁免），未做到 40×40。
- 选项切换只有 list/grid 两档；depth 只实现 moderate，1/3/4 的变量接口留着但不暴露成用户控件（避免把"降级设计"做成功能）。
- 规则变更时同步面：`usage()`、README 命令表、`skill/SKILL.md`、`DEPLOY.md`。

## 变更纪律

- 改命令/输出/校验规则 → 同步 `usage()`、README 命令表、`skill/SKILL.md`、`DEPLOY.md` 四处文档。
- 改 SPEC 行为（字段/禁令/状态机）→ 先改 `SPEC.md` 再改代码。
- **提交信息写详细**：首行 ≤50 字概括（动词开头），空行后列改动要点与原因，多 `-m` 或 heredoc；禁止"update files"式空话。仓库提交与 vault 提交同规范（vault 侧见 SKILL.md 维护段）。