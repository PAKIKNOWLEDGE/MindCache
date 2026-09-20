# MindCache

filesystem-first 个人知识库。AI Agent 是主要写入者，Markdown 文件是唯一数据源，
`mind` CLI 负责校验与生成静态 dashboard。

```
你说一句话 → Hermes（依据 skill/SKILL.md）→ mind new → ~/mind/
                              → mind check && mind build → ~/mind/dist/ → 浏览器
```

- 格式规范：[SPEC.md](SPEC.md)
- 部署与版本更新指南：[DEPLOY.md](DEPLOY.md)
- 开发进度对接：[AGENTS.md](AGENTS.md)
- Agent 操作手册（即 Hermes 的 skill）：[skill/SKILL.md](skill/SKILL.md)

## 命令

| 命令 | 作用 |
| --- | --- |
| `mind init` | 初始化 vault（默认 `~/mind`，含 git init） |
| `mind new <type> [标题]` | 创建条目（thought / todo / idea / note；`--body`/`--tags` 或管道 stdin 一步写正文标签） |
| `mind check` | lint 全部条目（文件名、frontmatter；type/目录不一致仅 WARN 不报错） |
| `mind build` | 生成静态站点到 `~/mind/dist/`（index + 分类页 + TAGS 聚合页 + search.json） |
| `mind search <关键词>` | 全库检索（标题/标签/正文，含 `archive/`；dashboard 搜索框同源） |
| `mind done <file>` / `mind reopen <file>` | todo 状态机（done 写完成日期，reopen 删 `done` 字段） |
| `mind archive <file>` | 归档到 `archive/`（frontmatter 不变） |
| `mind serve --port 8181 [--bind IP]` | 局域网提供 dist 访问 |

vault 位置：`$MIND_VAULT` 或 `--vault PATH` 覆盖，默认 `~/mind`。

## Dashboard 外观

`mind build` 产出的 dashboard 按本地 skill `ark-ui` 的契约实现，两个轴分开：

| 轴 | 取值 | 说明 |
| --- | --- | --- |
| family | `exa`（固定） | 午夜/纸白 + 水青信号色 + 衬线叙事字体；根属性 `data-ark-theme="exa"` |
| depth | `moderate`（固定） | shell 重组 + 舞台层（分布圆环 / 年表刻度 / 14 天捕获条）；根属性 `data-ark-depth="moderate"` |
| appearance | auto / light / dark | `data-ark-appearance`，跟随系统或手动，页头 `THEME 外观` 按钮切换 |
| layout | list / grid | `data-ark-layout`，页头 `VIEW 视图` 按钮切换（GRID 让"最近捕获"与"未完成待办"并排） |

- 样式在 `assets/ark/`：`appearance.css`（明暗）、`family-exa.css`（字体/几何/母题）、`depth-moderate.css`（层数与动效）、`base.css`（组件语法），构建时拼成 `dist/style.css`。
- 偏好存在 `localStorage`：`mind-appearance`、`mind-layout`；旧的 `mind-theme` / `mind-density` 首次打开时自动迁移并清理。
- 骨架：左侧竖栏导航（窄屏降级为底部动作条）、页面级舞台、`header/main/footer/nav` 地标、每页一个 `h1`、全局 `:focus-visible`、`prefers-reduced-motion` 有等价静态稿。

## NixOS 构建

```bash
nix-build -A mindcache          # 常规构建，产物 ./result/bin/mind
nix-build -A mindcache-musl     # 静态 musl 构建，单文件可拷贝到任意 Linux
nix-env -f . -iA mindcache      # 安装进用户 profile（PATH）
```

构建依赖 `Cargo.lock` 在仓库中（buildRustPackage 哈希校验需要）。

## 致谢与参考

- **[ark-ui](.grok/skills/ark-ui/SKILL.md)**（本地 skill）——dashboard 视觉契约的来源：family `exa`、depth `moderate` 的判定标准、`family / depth / appearance` 三轴分离、以及"装饰层必须承担分组/方向/状态/世界观之一"的审查规则。
- **[ignoredone.space · 终末地美术资源系统](https://www.ignoredone.space/index.php/endfield_design/)**——其官方封面视觉走查提供了构图参考：方括号标题、左缘刻度尺（本项目落到按 created 时间等比落位的年表刻度）、底边收边带。
- **HarmonyOS Sans SC**（© 2021 Huawei Device Co., Ltd.）——内嵌中文字体（GB2312 子集 woff2），授权条款见 [assets/fonts/LICENSE-HarmonyOS-Sans.txt](assets/fonts/LICENSE-HarmonyOS-Sans.txt)。
- **[dsh-theme-endfield](https://github.com/ymh0000123/dsh-theme-endfield)**（© 2026 ymh0000123，MIT）——早期一版 endfield 皮肤的设计语言来源；按 skill 的 family 判定（本产品无"现场任务/物流"语义，endfield 的 shell 签名无处兑现）该皮肤已整体移除，此处保留 MIT 出处记录。参考副本仅存于本地 `design-ref/`（已 gitignore，不进仓库）。

## 开发

```bash
nix develop -f shell.nix       # 本机无 rustc：进入开发环境（crates.io 已走国内镜像）
cargo build                    # 或在环境外直接跑 `nix develop -f shell.nix -c cargo build`
MIND_VAULT=/tmp/mind-dev cargo run -- init /tmp/mind-dev   # 沙箱验证，勿裸跑 init
```
