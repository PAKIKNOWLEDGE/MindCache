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
ls ~/mind/dist/                  # 应有 index.html + style.css + 4 个分页 + pages/
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

## 格式规范

一切读写必须遵守仓库根目录的 `SPEC.md`。修改任何文件前先读完它。
