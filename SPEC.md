# MindCache SPEC — 数据格式与规范 v0.1

MindCache 的数据层是纯文件系统：Markdown + frontmatter + 目录结构。
本文档是唯一权威格式定义。任何 Agent（Hermes / Codex / Claude Code / 人）读写 vault 都必须遵守本文档。

## 1. Vault 目录结构

默认位置 `~/mind/`（可用环境变量 `MIND_VAULT` 覆盖）：

```
~/mind/
├── inbox/      拿不准分类的内容，Agent 的兜底去处
├── todo/       待办（type: todo）
├── ideas/      想法、念头（type: thought / idea）
├── notes/      知识性内容、笔记（type: note）
└── archive/    归档，任何 type 都可以放进来
```

规则：

- 目录是粗分桶，**不承担精确分类职责**。检索靠全文搜索 + tags，不靠目录。
- 归错目录不是错误，`mv` 即可修复。
- 不要自行新增顶层目录。需要新类别时先改本 SPEC。

## 2. 文件名规则

```
YYYYMMDD-HHMM-短slug.md
例如：20260831-1840-agent-注意力分配.md
```

- 前缀是**本地时间**的创建时间戳，精确到分钟。
- slug：小写字母、数字、连字符，可保留 CJK 字符；其余符号丢弃。
- 时间戳前缀一旦写入**不可修改**——它是文件身份，也天然避免多 Agent 并发冲突。
- 重命名（改 slug）可以，但必须保留时间戳前缀。

## 3. Frontmatter Schema

YAML，紧跟文件开头的 `---` 块。

公共字段（所有 type 必填）：

| 字段     | 说明                                            |
| -------- | ----------------------------------------------- |
| `type`   | `thought` \| `todo` \| `idea` \| `note`         |
| `title`  | 标题，一行字符串，不空                            |
| `created`| ISO 8601，建议带本地时区偏移，如 `2026-08-31T18:40:00+08:00` |
| `tags`   | YAML 列表，自由填写，**无受控词表**；可为空 `[]`   |

todo 额外字段：

| 字段     | 说明                                    |
| -------- | --------------------------------------- |
| `status` | `open` \| `done`，缺省按 `open` 处理      |
| `due`    | 可选，`YYYY-MM-DD`                       |

示例：

```markdown
---
type: thought
title: AI 与注意力分配
created: 2026-08-31T18:40:00+08:00
tags:
  - ai
  - agent
---

我突然觉得个人 AI 真正改变的可能不是生产力，
而是人的注意力分配。
```

```markdown
---
type: todo
title: 修复 Sleephat 的 Ctrl+F
created: 2026-08-31T09:12:00+08:00
status: open
due: 2026-09-01
tags: []
---

拆键盘检查排线。
```

## 4. 硬性禁令（保护数据）

1. **不发明私有语法**。只用标准 Markdown + 本 SPEC 的 frontmatter。不引入 wiki-link、callout 等私有扩展。
2. **不修改 frontmatter 字段名**。加字段需要先改本 SPEC。
3. **不删除 `archive/` 内的内容**。
4. **不改时间戳前缀**。
5. 修改任何文件后必须通过 `mind check`。

## 5. 状态机

- `todo`：`open → done`（改 frontmatter 的 `status`）。完成的 todo 不挪目录、不删文件。
- 任何条目归档 = 移入 `archive/`，frontmatter 不变。

## 6. 派生物

`dist/`（`mind build` 的输出）、git 历史、任何索引/缓存都是**派生物**，可以随时删除重建。
Markdown 文件是唯一 source of truth。
