---
name: mind
description: 操作用户的个人知识库 MindCache（~/mind/）。当用户说"记一下/记录/我想到/加个待办/记个 todo/我之前是不是想过……"等捕获或检索请求时使用。
---

# MindCache — 个人知识库操作

用户的个人知识库 vault 位置以 `mind path` 的输出为准（默认 `~/mind/`，可通过 config 文件、`MIND_VAULT` 环境变量或 `--vault` 参数改变）。
数据层是纯 Markdown + YAML frontmatter，格式规范见仓库 `SPEC.md`。你是一个客户端，不是数据的所有者。

## 捕获（用户说"记一下……"）

1. 判断类型：
   - 明确要做的事、任务 → `todo`（写入 `todo/`）
   - 灵感、点子 → `idea`（写入 `ideas/`）
   - 一时的念头、观点、随想 → `thought`（写入 `ideas/`）
   - 知识、事实、整理过的信息 → `note`（写入 `notes/`）
   - **拿不准 → 写入 `inbox/`，type 取最接近的一个。** 宁可进 inbox，不要追问用户。
2. 创建文件：

   ```bash
   mind new <type> "标题"
   ```

   它会生成带时间戳文件名的模板文件并打印路径。然后用编辑操作把正文填进去：
   - `title` 用一句概括，不要把整句话塞进 title
   - `created` 已由模板生成，不要改
   - `tags` 打 1–3 个自由标签，宁缺勿滥
   - todo 加 `status: open`（模板已含），有明确截止时间才写 `due`
   - 正文保留用户的原话为主，可做轻度整理，不要虚构扩写
3. 校验并刷新视图：

   ```bash
   mind check && mind build
   ```

4. 向用户简短确认记了什么、放在哪。

## 检索（用户问"我之前是不是想过……"）

```bash
V="$(mind path)"
rg -l "关键词" "$V"          # 找文件
rg -i -C 2 "关键词" "$V"     # 带上下文
# rg 不可用时兜底：
grep -rn --include='*.md' "关键词" "$V"
```

- 多试几个同义关键词（中英文都试）。
- 目录归属不代表内容边界，全库搜索。
- 找到后直接把内容讲给用户，并给出文件路径。
- 搜不到就如实说搜不到，不要编造。

## 修改与归档

- 改 todo 状态：把 frontmatter 的 `status` 改为 `done`，不挪目录、不删文件。
- 归档：把文件 `mv` 进 `archive/`，frontmatter 不变。
- 用户说"整理一下 inbox"时：逐条读 `inbox/`，按上面的分类规则移到正确目录，改正缺失/错误的必填字段，最后跑 `mind check && mind build`，汇报移动了哪些。
- 修改后必须跑 `mind check && mind build`。

## 禁止

- 不改文件名里的时间戳前缀。
- 不改 frontmatter 字段名，不自创字段。
- 不删除 `archive/` 里的任何内容。
- 不发明私有语法（wiki-link、私有 callout 等），只用标准 Markdown。
- 不把 vault 数据写到配置的 vault 目录（`mind path`）之外。

## 维护

- vault 应当是 git 仓库（`mind init` 会初始化）。每次写完可以 `git -C "$(mind path)" add -A && git -C "$(mind path)" commit -m "<简述>"` 作为安全网；若 commit 因缺少 user.name/user.email 失败，告知用户配置一次即可，不要自行改全局 git 配置。
- dashboard 由 `mind build` 生成在 vault 的 `dist/` 目录下，`mind serve` 在局域网提供访问。
