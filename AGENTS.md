# AGENTS.md — MindCache 首次部署指南

本文件写给任何拥有 shell 权限、负责在目标机器上完成首次部署的 Agent（Hermes / Codex / Claude Code 等皆可）。
按顺序执行，每步完成后向用户简短汇报。

## 前提

- `mind` 命令已在 PATH 中。二进制的构建与安装是**用户的职责**，不是你的——先 `which mind` 确认；若不可用，如实告知用户并停止，不要自行构建或安装。
- 检索依赖 `rg`（ripgrep）——先 `which rg` 确认；若缺失，告知用户安装，不要自行安装。
- 你对用户 home 目录有读写权限。

## 第 1 步：初始化 vault

```bash
mind init        # 默认 ~/mind，可 mind init /某/路径 指定；位置会记录进 ~/.config/mind/config.toml
mind check       # 应显示 0 error（vault 为空时 0 ok 属正常）
```

`init` 会创建 `~/mind/{inbox,todo,ideas,notes,archive}`、执行 `git init`，并把 vault 位置写入 config。之后所有命令自动解析该位置，脚本用 `mind path` 获取。

## 第 2 步：写入第一条真实数据并生成视图

```bash
mind new idea "hello world"     # 记下部署完成的时刻
mind check; mind build
ls ~/mind/dist/                  # 应有 index.html + style.css + 4 个分页 + pages/ + fonts/
```

## 第 3 步：常驻服务（可选，局域网访问 dashboard）

创建 `~/.config/systemd/user/mind.service`：

```ini
[Unit]
Description=MindCache dashboard server

[Service]
# 路径假设经 nix-env 安装（~/.nix-profile/bin/mind）；
# vault 非默认位置时追加 --vault /实际/路径
ExecStart=%h/.nix-profile/bin/mind serve --port 8181
Restart=on-failure

[Install]
WantedBy=default.target
```

```bash
systemctl --user daemon-reload
systemctl --user enable --now mind.service
loginctl enable-linger $USER   # 注销后仍常驻
```

之后局域网内任意设备访问 `http://<机器IP>:8181`。

## 第 4 步：把操作手册接入你所在的 Agent

`skill/SKILL.md` 是 vault 的操作手册（捕获/检索/整理规则与禁令）。按你所在 Agent 框架的方式安装：

- 有 skill 目录/技能系统 → 把 `skill/SKILL.md` 复制或链接进去；
- 没有 skill 机制 → 把全文放进该 Agent 的系统提示词或等效的常驻指令文件。

验收对话：

- 用户说"记一下：……" → Agent 应创建文件、跑 `mind check && mind build`、向用户简短确认。
- 用户问"我之前是不是想过……" → Agent 应 rg 检索 `~/mind/` 并如实回答。

## 后续版本更新（用户要求时）

更新只做**源码仓库 + 服务重启**，vault 数据不动（数据流程见下）。

```bash
# ① 进到 T430 上存放 MindCache 源码的目录（当初 clone 的地方），先拉代码
cd <MindCache 源码目录> && git pull
# ② 看这次改了什么，再决定是否需要动 vault/SPEC（纯代码更新通常不需要）
git log --oneline -5
# ③ 重装二进制——注意：与首次部署同机制，用 nix-env，不是 nix profile！
nix-env -f . -iA mindcache      # 同名安装即覆盖升级，旧版进 profile 历史
# ④ 让常驻服务换上新二进制（ExecStart 指向 ~/.nix-profile/bin/mind 的当前代）
systemctl --user restart mind.service
```

验收：浏览器硬刷新（Ctrl+F5）dashboard，右下角 `MIND v<版本>` 与 `LAST BUILD` 时间应为新值；或 `mind --help` 首行核对版本。

**绝对不要做**：

- 不要用 `nix profile install/remove ./result` 之类 flake 命令装这个仓库——`default.nix` 是 channel 风格，只能用 `nix-env -f . -iA mindcache`。两套机制混用会各自维护 profile，PATH 里可能残留旧版 mind。
- 不要在 `git pull` 之前 build、看 diff 或改源码。
- 不要跳过第 ③ 步直接重启服务——重启不会自己拉新代码。
- 不要动 vault（`~/mind/`）里的文件来"配合更新"，除非 SPEC.md 有明确迁移要求。
- 不要动 `~/.config/mind/config.toml` 与 `mind.service` 文件本身。

**回滚**（新版本有问题时）：`nix-env --rollback` 退回上一代二进制，再 `systemctl --user restart mind.service`。

## vault 侧流程（与代码更新无关，随时可做）

```bash
cd ~/mind && git pull          # vault 本身是 git 仓库，拉最新数据
mind check && mind build       # 校验 + 重建 dashboard
```

需要对照最新 SPEC.md 人工检查的内容（类型名、字段变化）在 pull 后 `git log -p` 看 diff 判断，不要盲目批量改文件。

## 格式规范

一切读写必须遵守仓库根目录的 `SPEC.md`。修改任何文件前先读完它。
