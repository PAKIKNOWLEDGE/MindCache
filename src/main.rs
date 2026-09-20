use std::env;
use std::fs;
use std::io::{IsTerminal, Read, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::thread;

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, SecondsFormat, TimeZone};

const DIRS: [&str; 5] = ["inbox", "todo", "ideas", "notes", "archive"];
const TYPES: [&str; 4] = ["thought", "todo", "idea", "note"];
const VERSION: &str = env!("CARGO_PKG_VERSION");

mod render;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    match args[0].as_str() {
        "init" => {
            // mind init [PATH]：显式 --vault / MIND_VAULT 优先，否则取位置参数，再退默认
            let rest = &args[1..];
            let vault = explicit_vault(rest)
                .or_else(|| {
                    env::var("MIND_VAULT")
                        .ok()
                        .filter(|s| !s.is_empty())
                        .map(PathBuf::from)
                })
                .unwrap_or_else(|| {
                    positionals(rest)
                        .first()
                        .map(|p| PathBuf::from(*p))
                        .unwrap_or_else(default_vault)
                });
            cmd_init(&vault);
        }
        "new" => {
            let rest = &args[1..];
            let vault = extract_vault(rest);
            let inbox = rest.iter().any(|a| a == "--inbox");
            // positional args: type, title...
            let pos: Vec<&String> = positionals(rest);
            if pos.is_empty() {
                eprintln!("mind new <type> [title] [--body TEXT] [--tags a,b]  — type: idea|todo|note（--inbox 放入 inbox/）");
                exit(2);
            }
            let type_ = pos[0].to_string();
            let title = if pos.len() > 1 {
                pos[1..].iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ")
            } else {
                String::new()
            };
            let body = extract_body(rest);
            let tags = extract_tags(rest);
            cmd_new(&vault, &type_, &title, &body, &tags, inbox);
        }
        "check" => cmd_check(extract_vault(&args[1..]), positional_path(&args[1..])),
        "build" => cmd_build(extract_vault(&args[1..])),
        "search" => {
            let vault = extract_vault(&args[1..]);
            let query = positionals(&args[1..])
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            if query.is_empty() {
                eprintln!("mind search <query>  — 检索整个 vault（标题/标签/正文，含 archive）");
                exit(2);
            }
            cmd_search(vault, &query);
        }
        "done" | "reopen" => {
            let vault = extract_vault(&args[1..]);
            let p = positional_path(&args[1..]).unwrap_or_else(|| {
                die("需要指定条目路径，如: mind done todo/20260909-0930-fix-keyboard.md")
            });
            cmd_state(&vault, &args[0], &p);
        }
        "archive" => {
            let vault = extract_vault(&args[1..]);
            let p = positional_path(&args[1..]).unwrap_or_else(|| {
                die("需要指定条目路径，如: mind archive notes/20260909-0930-x.md")
            });
            cmd_archive(&vault, &p);
        }
        "serve" => {
            let vault = extract_vault(&args[1..]);
            let port = extract_port(&args[1..]);
            let bind = extract_bind(&args[1..]);
            cmd_serve(vault, port, bind);
        }
        "path" => println!("{}", extract_vault(&args[1..]).display()),
        "help" | "--help" | "-h" => usage(),
        _ => usage(),
    }
}

/// 带值的 flag 名；positionals 跳过它们及其后的值 token
const FLAG_WITH_VALUE: [&str; 7] = ["--vault", "-v", "--port", "-p", "-b", "--body", "--tags"];

/// 提取位置参数，正确跳过 flag 及其值（--vault X / --vault=X / -p 8080）
fn positionals(args: &[String]) -> Vec<&String> {
    let mut out = Vec::new();
    let mut skip_next = false;
    for a in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if FLAG_WITH_VALUE.contains(&a.as_str()) {
            skip_next = true;
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            continue; // 无值 flag（--inbox 等）或 --flag=value 形式
        }
        out.push(a);
    }
    out
}

fn explicit_vault(args: &[String]) -> Option<PathBuf> {
    for (i, a) in args.iter().enumerate() {
        if let Some(v) = a.strip_prefix("--vault=") {
            return Some(PathBuf::from(v));
        }
        if a == "--vault" || a == "-v" {
            if let Some(p) = args.get(i + 1) {
                return Some(PathBuf::from(p));
            }
        }
    }
    None
}

fn default_vault() -> PathBuf {
    home_dir().join("mind")
}

fn usage() -> ! {
    println!(
        "mind v{VERSION} — MindCache: filesystem-first personal knowledge base

USAGE:
  mind init [PATH]              initialize a vault (default ~/mind), git init included
  mind new <type> [TITLE]       create an entry (type: idea|todo|note; --inbox puts it in inbox/)
                                --body TEXT 或管道 stdin 作为正文；--tags a,b 打标签
  mind check [FILE.md]          lint vault entries (or a single file)
  mind build                    generate static dashboard into <vault>/dist/
  mind search <query>           search titles/tags/body across the whole vault (incl. archive)
  mind done <file>              mark a todo entry done (writes done: YYYY-MM-DD)
  mind reopen <file>            reopen a done todo entry
  mind archive <file>           move an entry into archive/
  mind serve [--port N] [--bind IP]  serve <vault>/dist/ over LAN (default port 8181, bind 0.0.0.0)
  mind path                     print the resolved vault location

Vault location precedence: --vault PATH > $MIND_VAULT > ~/.config/mind/config.toml > ~/mind."
    );
    exit(0);
}

// ---------------------------------------------------------------- config

/// ~/.config/mind/config.toml（尊重 XDG_CONFIG_HOME）
fn config_file() -> PathBuf {
    let base = env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".config"));
    base.join("mind").join("config.toml")
}

/// 从 config 读 vault 路径（只认 `vault = "..."` 行，其余键留给未来扩展）
fn config_vault() -> Option<PathBuf> {
    let text = fs::read_to_string(config_file()).ok()?;
    for line in text.lines() {
        let t = line.trim();
        if let Some(v) = t.strip_prefix("vault") {
            let v = v.trim().strip_prefix('=')?.trim();
            let v = v.trim_matches('"').trim_matches('\'');
            if !v.is_empty() {
                return Some(PathBuf::from(v));
            }
        }
    }
    None
}

/// 记录 vault 位置到 config（已有文件则只替换/追加 vault 行）
fn save_config_vault(vault: &Path) {
    let cf = config_file();
    if let Some(parent) = cf.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|e| die(&format!("写配置失败（无法创建 {}）: {e}", parent.display())));
    }
    let new_line = format!("vault = \"{}\"", vault.display());
    let content = fs::read_to_string(&cf).unwrap_or_default();
    let mut replaced = false;
    let mut out: Vec<String> = content
        .lines()
        .map(|l| {
            if l.trim_start().starts_with("vault") {
                replaced = true;
                new_line.clone()
            } else {
                l.to_string()
            }
        })
        .collect();
    if !replaced {
        if !out.is_empty() {
            out.push(String::new()); // 与已有内容空一行
        }
        out.push("# MindCache vault location (resolved by `mind path`)".into());
        out.push(new_line);
    }
    fs::write(&cf, out.join("\n") + "\n")
        .unwrap_or_else(|e| die(&format!("写配置失败 ({}): {e}", cf.display())));
}

fn extract_vault(args: &[String]) -> PathBuf {
    explicit_vault(args)
        .or_else(|| {
            env::var("MIND_VAULT")
                .ok()
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
        })
        .or_else(config_vault)
        .unwrap_or_else(default_vault)
}

fn extract_bind(args: &[String]) -> String {
    for (i, a) in args.iter().enumerate() {
        if let Some(v) = a.strip_prefix("--bind=") {
            if !v.is_empty() {
                return v.to_string();
            }
        }
        if a == "--bind" {
            if let Some(p) = args.get(i + 1) {
                if !p.is_empty() {
                    return p.clone();
                }
            }
        }
    }
    "0.0.0.0".to_string()
}

fn extract_port(args: &[String]) -> u16 {
    if let Some(i) = args.iter().position(|a| a == "--port" || a == "-p") {
        if let Some(p) = args.get(i + 1) {
            if let Ok(n) = p.parse() {
                return n;
            }
        }
    }
    8181
}

fn positional_path(args: &[String]) -> Option<PathBuf> {
    positionals(args).first().map(|p| PathBuf::from(*p))
}

fn home_dir() -> PathBuf {
    env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

// ---------------------------------------------------------------- frontmatter

#[derive(Debug, Clone, Default)]
struct Fm {
    type_: String,
    title: String,
    created: String,
    status: Option<String>,
    due: Option<String>,
    done: Option<String>,
    tags: Vec<String>,
}

#[derive(Debug, Clone)]
struct Entry {
    dir: String,   // folder name inside vault
    stem: String,  // filename without .md
    fm: Fm,
    body: String,
}

/// Split a markdown file into (frontmatter, body). Returns None if no well-formed block.
/// 容忍开头 `--- ` 的尾随空白、\r\n、结束符行前后的空白。
fn split_fm(text: &str) -> Option<(&str, &str)> {
    let t = text.trim_start_matches('\u{feff}');
    let rest = t.strip_prefix("---")?;
    let nl = rest.find('\n')?;
    if !rest[..nl].trim_end().is_empty() {
        return None; // "---xxx" 不是 frontmatter 开头
    }
    let after_first = &rest[nl + 1..];
    // 逐行找结束符：trim_end 后恰为 "---" 的行（兼容 \r\n 与尾随空格）
    let mut end = None;
    let mut off = 0usize;
    loop {
        let line = &after_first[off..];
        if line.is_empty() {
            break;
        }
        match line.find('\n') {
            Some(n) => {
                if line[..n].trim_end() == "---" {
                    end = Some(off);
                    break;
                }
                off += n + 1;
            }
            None => {
                // 无换行的最后一行
                if line.trim_end() == "---" {
                    end = Some(off);
                }
                break;
            }
        }
    }
    let end = end?;
    let fm = &after_first[..end];
    let after = &after_first[end + 3..]; // 跳过 "---"
    let body = after
        .strip_prefix('\n')
        .or_else(|| after.strip_prefix("\r\n"))
        .unwrap_or(after);
    Some((fm, body))
}

/// 只剥成对引号：首尾为同一引号且长度 ≥2 时去掉外层，否则原样。
/// 例：`"好"` → `好`，`他说的"好"` → `他说的"好"`（不拆内部引号）。
fn strip_paired_quotes(s: &str) -> &str {
    if s.len() >= 2 {
        let b = s.as_bytes();
        if (b[0] == b'"' && b[s.len() - 1] == b'"') || (b[0] == b'\'' && b[s.len() - 1] == b'\'') {
            return &s[1..s.len() - 1];
        }
    }
    s
}

/// 行内 flow 列表 `[a, "b,c", d]` 的拆分：逗号切分，双引号包裹内的逗号不拆；
/// 逐项剥成对引号，空项丢弃。
fn split_tags_flow(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_dq = false;
    for c in s.chars() {
        match c {
            '"' => {
                in_dq = !in_dq;
                cur.push(c);
            }
            ',' if !in_dq => {
                let t = strip_paired_quotes(cur.trim());
                if !t.is_empty() {
                    out.push(t.to_string());
                }
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    let t = strip_paired_quotes(cur.trim());
    if !t.is_empty() {
        out.push(t.to_string());
    }
    out
}

fn parse_fm(text: &str) -> Result<Fm, String> {
    let (block, _) = split_fm(text).ok_or("缺少 frontmatter 块（文件需以 --- 开头）")?;
    let mut fm = Fm::default();
    let mut in_tags = false;
    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if in_tags {
            if let Some(item) = trimmed.strip_prefix("- ") {
                fm.tags.push(item.trim().to_string());
                continue;
            } else {
                in_tags = false;
            }
        }
        let (k, v) = match trimmed.split_once(':') {
            Some(kv) => kv,
            None => return Err(format!("frontmatter 行无法解析: \"{trimmed}\"")),
        };
        let k = k.trim();
        let v = strip_paired_quotes(v.trim()).to_string();
        match k {
            "type" => fm.type_ = v,
            "title" => fm.title = v,
            "created" => fm.created = v,
            "status" => fm.status = Some(v),
            "due" => fm.due = Some(v),
            "done" => fm.done = Some(v),
            "tags" => {
                // 支持 YAML 块式列表（后续 "- item" 行）与行内 flow 列表 [a, b]
                if v == "[]" {
                    fm.tags = Vec::new();
                    in_tags = false;
                } else if v.starts_with('[') {
                    fm.tags = split_tags_flow(v[1..].strip_suffix(']').unwrap_or(&v[1..]));
                    in_tags = false;
                } else {
                    in_tags = true;
                }
            }
            _ => {} // 未知字段容忍读取，check 不强制
        }
    }
    if fm.type_ == "thought" {
        fm.type_ = "idea".to_string(); // SPEC: thought 是 idea 的历史别名
    }
    Ok(fm)
}

fn parse_created(s: &str) -> Option<i64> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp());
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(Local.from_utc_datetime(&dt).timestamp());
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp());
    }
    None
}

fn fmt_created(s: &str) -> String {
    // 展示用：尽量转成 YYYY-MM-DD HH:MM
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return dt.format("%Y-%m-%d %H:%M").to_string();
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return dt.format("%Y-%m-%d %H:%M").to_string();
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return d.format("%Y-%m-%d").to_string();
    }
    s.to_string()
}

fn valid_filename(stem: &str) -> bool {
    if !stem.is_ascii() {
        return false; // SPEC: 文件名仅限 ASCII
    }
    let b = stem.as_bytes();
    if b.len() < 14 {
        return false;
    }
    if !(b[..8].iter().all(u8::is_ascii_digit)
        && b[8] == b'-'
        && b[9..13].iter().all(u8::is_ascii_digit)
        && b[13] == b'-')
    {
        return false;
    }
    // SPEC: slug 仅限小写字母、数字、连字符（保证 href/URL 安全）
    let slug = &stem[14..];
    !slug.is_empty()
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn type_dir(type_: &str) -> Option<&'static str> {
    match type_ {
        "todo" => Some("todo"),
        "idea" | "thought" => Some("ideas"),
        "note" => Some("notes"),
        _ => None,
    }
}

fn read_entries(vault: &Path, dirs: &[&str]) -> (Vec<Entry>, Vec<(String, String)>) {
    let mut entries = Vec::new();
    let mut errors = Vec::new();
    for dir in dirs {
        let dp = vault.join(dir);
        let rd = match fs::read_dir(&dp) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let mut paths: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        paths.sort();
        for p in paths {
            let name = match p.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if !name.ends_with(".md") || p.is_dir() {
                continue;
            }
            let stem = name.trim_end_matches(".md").to_string();
            let text = match fs::read_to_string(&p) {
                Ok(t) => t,
                Err(e) => {
                    errors.push((format!("{dir}/{name}"), format!("无法读取: {e}")));
                    continue;
                }
            };
            match parse_fm(&text) {
                Ok(fm) => entries.push(Entry {
                    dir: dir.to_string(),
                    stem,
                    fm,
                    body: split_fm(&text).map(|(_, b)| b.to_string()).unwrap_or_default(),
                }),
                Err(e) => errors.push((format!("{dir}/{name}"), e)),
            }
        }
    }
    (entries, errors)
}

// ---------------------------------------------------------------- init

fn cmd_init(vault: &Path) {
    if vault.exists() {
        println!("vault 已存在: {}", vault.display());
    } else {
        fs::create_dir_all(&vault).unwrap_or_else(|e| die(&format!("创建目录失败: {e}")));
    }
    for d in DIRS {
        let dp = vault.join(d);
        if !dp.exists() {
            fs::create_dir_all(&dp).unwrap_or_else(|e| die(&format!("创建 {d}/ 失败: {e}")));
        }
    }
    let gi = vault.join(".gitignore");
    if !gi.exists() {
        fs::write(&gi, "dist/\n").ok();
    }
    // README 占位，保证 git 仓库非空
    let rm = vault.join("README.md");
    if !rm.exists() {
        fs::write(&rm, "# MindCache Vault\n\n个人知识库。格式见 MindCache 项目的 SPEC.md。\n").ok();
    }
    let inited = std::process::Command::new("git")
        .arg("init")
        .current_dir(&vault)
        .output();
    match inited {
        Ok(o) if o.status.success() => {
            println!("git 仓库已初始化");
            // 新机器常缺 user.name/email，agent 的安全网 commit 会因此失败；
            // 仅在完全未配置时写 repo-local 兜底身份（不碰全局配置）
            for (k, v) in [("user.name", "mindcache"), ("user.email", "mindcache@localhost")] {
                let has = std::process::Command::new("git")
                    .args(["config", k])
                    .current_dir(&vault)
                    .output()
                    .map(|o| o.status.success() && !o.stdout.is_empty())
                    .unwrap_or(false);
                if !has {
                    let _ = std::process::Command::new("git")
                        .args(["config", k, v])
                        .current_dir(&vault)
                        .output();
                    println!("已写 repo-local git 兜底身份 {k}={v}（如需真实身份可 git config 覆盖）");
                }
            }
        }
        _ => println!("（git 不可用或已存在仓库，跳过 git init）"),
    }
    save_config_vault(vault);
    println!("vault 位置已记录到 {}", config_file().display());
    println!("vault 就绪: {}（后续命令自动解析，脚本可用 mind path 获取）", vault.display());
    println!("下一步: mind new idea \"hello world\"");
}

// ---------------------------------------------------------------- new

fn slugify(s: &str) -> String {
    // SPEC: slug 仅限 ASCII（小写字母、数字、连字符），非 ASCII 字符丢弃
    let mut out = String::new();
    let mut prev_dash = true; // 抑制开头连字符
    for c in s.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// 从 new 参数里取正文：优先 `--body=…` / `-b=…` / `--body`/`-b` 后一 token；
/// 都没有且 stdin 非终端（管道/重定向/heredoc）时把 stdin 全文读作正文。
fn extract_body(args: &[String]) -> String {
    for (i, a) in args.iter().enumerate() {
        if let Some(v) = a.strip_prefix("--body=") {
            return v.to_string();
        }
        if let Some(v) = a.strip_prefix("-b=") {
            return v.to_string();
        }
        if a == "--body" || a == "-b" {
            return args.get(i + 1).cloned().unwrap_or_default();
        }
    }
    if !std::io::stdin().is_terminal() {
        let mut buf = String::new();
        let _ = std::io::stdin().read_to_string(&mut buf);
        return buf.trim_end_matches(['\n', '\r']).to_string();
    }
    String::new()
}

/// 从 new 参数里取 tags：`--tags=…` 或 `--tags` 后一 token，flow 拆分（引号内逗号不拆）。
fn extract_tags(args: &[String]) -> Vec<String> {
    for (i, a) in args.iter().enumerate() {
        if let Some(v) = a.strip_prefix("--tags=") {
            return split_tags_flow(v);
        }
        if a == "--tags" {
            return args.get(i + 1).map(|s| split_tags_flow(s)).unwrap_or_default();
        }
    }
    Vec::new()
}

fn cmd_new(vault: &Path, type_: &str, title: &str, body: &str, tags: &[String], inbox: bool) {
    let type_ = if type_ == "thought" { "idea" } else { type_ }; // 历史别名
    if !TYPES.contains(&type_) {
        eprintln!("未知类型 \"{type_}\"，可选: idea | todo | note");
        exit(2);
    }
    // --inbox：Agent 拿不准时落入 inbox/，文件名与 frontmatter 仍由工具负责
    let dir = if inbox { "inbox" } else { type_dir(type_).unwrap() };
    let dp = vault.join(dir);
    fs::create_dir_all(&dp).unwrap_or_else(|e| die(&format!("创建 {dir}/ 失败: {e}")));

    let now = Local::now();
    let stamp = now.format("%Y%m%d-%H%M");
    let slug = {
        let s = slugify(title);
        if s.is_empty() {
            type_.to_string()
        } else {
            s.chars().take(40).collect()
        }
    };
    let mut name = format!("{stamp}-{slug}.md");
    let mut path = dp.join(&name);
    let mut n = 1;
    while path.exists() {
        name = format!("{stamp}-{slug}-{n}.md");
        path = dp.join(&name);
        n += 1;
    }

    let created = now.to_rfc3339_opts(SecondsFormat::Secs, false);
    let tags_line = if tags.is_empty() {
        "tags: []".to_string()
    } else {
        format!("tags: [{}]", tags.join(", "))
    };
    let fm = if type_ == "todo" {
        format!(
            "---\ntype: todo\ntitle: {title}\ncreated: {created}\nstatus: open\n{tags_line}\n---\n\n"
        )
    } else {
        format!(
            "---\ntype: {type_}\ntitle: {title}\ncreated: {created}\n{tags_line}\n---\n\n"
        )
    };
    let content = if body.is_empty() {
        fm
    } else {
        format!("{fm}{body}\n")
    };
    fs::write(&path, content).unwrap_or_else(|e| die(&format!("写入失败: {e}")));
    println!("{}", path.display());
}

// ---------------------------------------------------------------- check

/// 单条目校验：返回 (errs, warns)。errs 是数据完整性/格式错误；warns 仅用于
/// 目录与 type 不一致（SPEC §1：目录是粗分桶，归错目录不是错误，可随时 mv 调整）。
fn check_entry(dir: &str, stem: &str, fm: &Fm) -> (Vec<String>, Vec<String>) {
    let mut errs = Vec::new();
    let mut warns = Vec::new();
    if !valid_filename(stem) {
        errs.push("文件名不符合 YYYYMMDD-HHMM-ascii-slug 格式（仅限 ASCII）".into());
    }
    if !TYPES.contains(&fm.type_.as_str()) {
        errs.push(format!("type 无效: \"{}\"", fm.type_));
    }
    if fm.title.trim().is_empty() {
        errs.push("title 缺失或为空".into());
    }
    if fm.created.is_empty() {
        errs.push("created 缺失".into());
    } else if parse_created(&fm.created).is_none() {
        errs.push(format!("created 无法解析: \"{}\"", fm.created));
    }
    if fm.type_ == "todo" {
        if let Some(st) = &fm.status {
            if st != "open" && st != "done" {
                errs.push(format!("status 无效: \"{st}\"（应为 open|done）"));
            }
        }
        if let Some(due) = &fm.due {
            if NaiveDate::parse_from_str(due, "%Y-%m-%d").is_err() {
                errs.push(format!("due 无法解析: \"{due}\"（应为 YYYY-MM-DD）"));
            }
        }
        if let Some(d) = &fm.done {
            if NaiveDate::parse_from_str(d, "%Y-%m-%d").is_err() {
                errs.push(format!("done 无法解析: \"{d}\"（应为 YYYY-MM-DD）"));
            }
        }
    }
    // type 与目录一致性：映射目录 / inbox / archive 皆合法；不一致只是 WARN
    if let Some(expect) = type_dir(&fm.type_) {
        if dir != expect && dir != "inbox" && dir != "archive" {
            warns.push(format!("type {} 一般放 {expect}/，当前在 {dir}/", fm.type_));
        }
    }
    (errs, warns)
}

fn cmd_check(vault: PathBuf, single: Option<PathBuf>) {
    let mut errs: Vec<(String, String)> = Vec::new();
    let mut warns: Vec<(String, String)> = Vec::new();
    let mut ok = 0usize;

    match single {
        Some(p) => {
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            let stem = name.trim_end_matches(".md").to_string();
            let dir = p
                .parent()
                .and_then(|d| d.file_name())
                .map(|d| d.to_string_lossy().to_string())
                .unwrap_or_default();
            match fs::read_to_string(&p) {
                Ok(text) => match parse_fm(&text) {
                    Ok(fm) => {
                        let (es, ws) = check_entry(&dir, &stem, &fm);
                        if es.is_empty() {
                            ok += 1;
                        } else {
                            for e in es {
                                errs.push((name.clone(), e));
                            }
                        }
                        for w in ws {
                            warns.push((name.clone(), w));
                        }
                    }
                    Err(e) => errs.push((name, e)),
                },
                Err(e) => errs.push((name, format!("无法读取: {e}"))),
            }
        }
        None => {
            // vault 存在性
            if !vault.is_dir() {
                eprintln!("vault 不存在: {}（先运行 mind init）", vault.display());
                exit(2);
            }
            let (entries, parse_errs) = read_entries(&vault, &DIRS);
            for (f, e) in &parse_errs {
                errs.push((f.clone(), e.clone()));
            }
            for e in &entries {
                let rel = format!("{}/{}.md", e.dir, e.stem);
                let (es, ws) = check_entry(&e.dir, &e.stem, &e.fm);
                if es.is_empty() {
                    ok += 1;
                } else {
                    for m in es {
                        errs.push((rel.clone(), m));
                    }
                }
                for w in ws {
                    warns.push((rel.clone(), w));
                }
            }
        }
    }

    for (f, e) in &errs {
        println!("ERR {f}: {e}");
    }
    for (f, w) in &warns {
        println!("WARN {f}: {w}");
    }
    println!(
        "checked: {ok} ok, {} error(s), {} warning(s), vault: {}",
        errs.len(),
        warns.len(),
        vault.display()
    );
    if !errs.is_empty() {
        exit(1);
    }
}

// ---------------------------------------------------------------- state machine

/// 把用户给的条目路径解析为 vault 内的规范 .md 绝对路径。
/// 顺序：绝对路径直用；否则 cwd 下存在；再 vault 下存在。校验落在 vault 内。
fn resolve_entry_path(vault: &Path, p: &Path) -> Result<PathBuf, String> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if p.is_absolute() {
        candidates.push(p.to_path_buf());
    } else {
        candidates.push(env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(p));
        candidates.push(vault.join(p));
    }
    for c in &candidates {
        if !c.exists() {
            continue;
        }
        if c.extension().and_then(|e| e.to_str()) != Some("md") {
            return Err(format!("不是 .md 文件: {}", c.display()));
        }
        let canon = c
            .canonicalize()
            .map_err(|e| format!("无法解析路径 {}: {e}", c.display()))?;
        let vc = vault
            .canonicalize()
            .map_err(|e| format!("vault 无法解析: {e}"))?;
        if !canon.starts_with(&vc) {
            return Err(format!("文件不在 vault 内: {}", c.display()));
        }
        return Ok(canon);
    }
    Err(format!("未找到条目: {}", p.display()))
}

fn write_fm_block(path: &Path, lines: &[String], body: &str) {
    let text = format!("---\n{}\n---\n{}", lines.join("\n"), body);
    fs::write(path, text).unwrap_or_else(|e| die(&format!("写入失败 ({}): {e}", path.display())));
}

/// done / reopen：按行重写 frontmatter，其余行原样保留（SPEC §5 状态机）。
fn cmd_state(vault: &Path, cmd: &str, p: &Path) {
    let path = match resolve_entry_path(vault, p) {
        Ok(x) => x,
        Err(e) => die(&e),
    };
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| die(&format!("读取失败 ({}): {e}", path.display())));
    let (fm, body) =
        split_fm(&text).unwrap_or_else(|| die(&format!("缺少 frontmatter 块: {}", path.display())));
    let parsed = parse_fm(&text).unwrap_or_else(|e| die(&format!("{}: {e}", path.display())));
    if parsed.type_ != "todo" {
        die("不是 todo 条目（仅 todo 支持 done / reopen）");
    }
    let is_done = parsed.status.as_deref() == Some("done");
    match cmd {
        "done" => {
            if is_done {
                die("该条目已经是 done 状态");
            }
            let today = Local::now().format("%Y-%m-%d").to_string();
            let mut out: Vec<String> = Vec::new();
            let mut status_seen = false;
            let mut done_seen = false;
            for l in fm.lines() {
                match l.split_once(':').map(|(k, _)| k.trim()) {
                    Some("status") => {
                        out.push("status: done".into());
                        status_seen = true;
                    }
                    Some("done") => {
                        out.push(l.to_string());
                        done_seen = true;
                    }
                    _ => out.push(l.to_string()),
                }
            }
            if !status_seen {
                let at = out
                    .iter()
                    .position(|l| l.split_once(':').map(|(k, _)| k.trim()) == Some("type"))
                    .map(|i| i + 1)
                    .unwrap_or(0);
                out.insert(at, "status: done".into());
            }
            if !done_seen {
                let at = out
                    .iter()
                    .position(|l| l.split_once(':').map(|(k, _)| k.trim()) == Some("status"))
                    .map(|i| i + 1)
                    .unwrap_or(out.len());
                out.insert(at, format!("done: {today}"));
            }
            write_fm_block(&path, &out, body);
            println!("{} -> done ({today})", path.display());
        }
        "reopen" => {
            if !is_done {
                die("该条目不是 done 状态");
            }
            let mut out: Vec<String> = Vec::new();
            let mut status_seen = false;
            for l in fm.lines() {
                match l.split_once(':').map(|(k, _)| k.trim()) {
                    Some("status") => {
                        out.push("status: open".into());
                        status_seen = true;
                    }
                    Some("done") => {} // SPEC §5：重开删除 done 字段
                    _ => out.push(l.to_string()),
                }
            }
            if !status_seen {
                let at = out
                    .iter()
                    .position(|l| l.split_once(':').map(|(k, _)| k.trim()) == Some("type"))
                    .map(|i| i + 1)
                    .unwrap_or(0);
                out.insert(at, "status: open".into());
            }
            write_fm_block(&path, &out, body);
            println!("{} -> open（已删除 done 日期）", path.display());
        }
        _ => unreachable!(),
    }
}

/// 归档：移入 vault/archive/（frontmatter 不变）。已在 archive/ 则提示返回。
fn cmd_archive(vault: &Path, p: &Path) {
    let path = match resolve_entry_path(vault, p) {
        Ok(x) => x,
        Err(e) => die(&e),
    };
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let in_archive = path
        .parent()
        .and_then(|d| d.file_name())
        .map(|d| d == "archive")
        .unwrap_or(false);
    if in_archive {
        println!("已在 archive/: {}", path.display());
        return;
    }
    let target = vault.join("archive").join(&name);
    if target.exists() {
        die(&format!("archive/ 已存在同名文件: {}", target.display()));
    }
    fs::create_dir_all(vault.join("archive"))
        .unwrap_or_else(|e| die(&format!("创建 archive/ 失败: {e}")));
    fs::rename(&path, &target).unwrap_or_else(|e| die(&format!("移动失败: {e}")));
    println!("已归档: {} -> {}", path.display(), target.display());
}

// ---------------------------------------------------------------- search

fn cmd_search(vault: PathBuf, query: &str) {
    if !vault.is_dir() {
        eprintln!("vault 不存在: {}（先运行 mind init）", vault.display());
        exit(2);
    }
    let (entries, errs) = read_entries(&vault, &DIRS);
    for (f, e) in &errs {
        eprintln!("WARN {f}: {e}");
    }
    let needle = query.to_lowercase();
    let mut hits = 0usize;
    for e in &entries {
        let hay = format!("{} {} {}", e.fm.title, e.fm.tags.join(" "), e.body).to_lowercase();
        if !hay.contains(&needle) {
            continue;
        }
        hits += 1;
        // 上下文：body 中含 needle 的最多 3 行
        let ctx: Vec<String> = e
            .body
            .lines()
            .filter(|l| l.to_lowercase().contains(&needle))
            .take(3)
            .map(|l| l.trim().to_string())
            .collect();
        println!("{}/{}", e.dir, format!("{}.md", e.stem));
        println!("  {} // {}", e.fm.title, fmt_created(&e.fm.created));
        for l in ctx {
            println!("  > {l}");
        }
    }
    println!("searched {} files, {hits} match(es)", entries.len());
}

// ---------------------------------------------------------------- build

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            _ => o.push(c),
        }
    }
    o
}

fn render_md(body: &str) -> String {
    let mut opts = comrak::ComrakOptions::default();
    // GFM 扩展：表格/删除线/自动链接，否则按字面渲染（~~x~~、管道表、裸 URL）
    opts.extension.table = true;
    opts.extension.strikethrough = true;
    opts.extension.autolink = true;
    comrak::markdown_to_html(body, &opts)
}

// 主题四态：auto（跟随系统）/ dark / light / endfield（工业印刷风参考 design-ref/）。
// concat! 只接受字面量，token 各出现两份（:root 基准 + 显式覆盖），改色值时同步所有块。
// HarmonyOS Sans SC（华为，免费商用授权，随本二进制再分发须保留 LICENSE）
// 内嵌的是**子集**：GB2312 全部汉字 + ASCII + 常用符号（约 7500 字，woff2 各 <1MB）。
// 子集外字符（生僻字/emoji）由浏览器按字符回退到栈内下一字体，不会缺字空白。
// 若需扩充字符集：用 fontTools.subset 以 --text-file 重新生成后替换 assets/fonts/ 下文件
const FONT_REGULAR: &[u8] = include_bytes!("../assets/fonts/HarmonyOS_Sans_SC_Regular.woff2");
const FONT_BOLD: &[u8] = include_bytes!("../assets/fonts/HarmonyOS_Sans_SC_Bold.woff2");
const FONT_LICENSE: &[u8] = include_bytes!("../assets/fonts/LICENSE-HarmonyOS-Sans.txt");


fn is_done_entry(e: &Entry) -> bool {
    e.fm.type_ == "todo" && e.fm.status.as_deref() == Some("done")
}

fn is_open_todo(e: &&Entry) -> bool {
    e.fm.type_ == "todo" && e.fm.status.as_deref().unwrap_or("open") != "done"
}

fn sort_by_created(entries: &mut [Entry]) {
    entries.sort_by_key(|e| std::cmp::Reverse(parse_created(&e.fm.created).unwrap_or(0)));
}

/// JSON 字符串转义：`"`、`\` 与 <0x20 控制字符（含 \n）转义；其余字符（含中文）原样保留。
fn json_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\u{08}' => o.push_str("\\b"),
            '\u{09}' => o.push_str("\\t"),
            '\u{0A}' => o.push_str("\\n"),
            '\u{0C}' => o.push_str("\\f"),
            '\u{0D}' => o.push_str("\\r"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            _ => o.push(c),
        }
    }
    o
}

fn cmd_build(vault: PathBuf) {
    if !vault.is_dir() {
        eprintln!("vault 不存在: {}（先运行 mind init）", vault.display());
        exit(2);
    }
    let (mut entries, errs) = read_entries(&vault, &DIRS);
    for (f, e) in &errs {
        eprintln!("警告: {f}: {e}（该文件未纳入视图，先运行 mind check）");
    }
    let built = Local::now().format("%Y-%m-%d %H:%M").to_string();
    let today = Local::now().format("%Y-%m-%d").to_string();

    let dist = vault.join("dist");
    let pages = dist.join("pages");
    let _ = fs::remove_dir_all(&pages);
    fs::create_dir_all(&pages).unwrap_or_else(|e| die(&format!("创建 dist 失败: {e}")));
    fs::write(dist.join("style.css"), render::CSS).unwrap();
    // 字体目录整目录重建：改版（如 TTF→woff2）后 dist 内不残留旧格式文件。
    // @font-face 引用相对 style.css 的 fonts/，详情页在 pages/ 也经 ../style.css 指向同一目录。
    let fonts_dir = dist.join("fonts");
    let _ = fs::remove_dir_all(&fonts_dir);
    fs::create_dir_all(&fonts_dir).unwrap_or_else(|e| die(&format!("创建 fonts 失败: {e}")));
    fs::write(fonts_dir.join("HarmonyOS_Sans_SC_Regular.woff2"), FONT_REGULAR).unwrap();
    fs::write(fonts_dir.join("HarmonyOS_Sans_SC_Bold.woff2"), FONT_BOLD).unwrap();
    fs::write(fonts_dir.join("LICENSE-HarmonyOS-Sans.txt"), FONT_LICENSE).unwrap();

    sort_by_created(&mut entries);
    let total = entries.len();
    let count_line =
        format!("<b class=\"num\">{total}</b> <span class=\"en\">ENTRIES</span> 条");

    // ---- 库内分布：圆环、图例、状态读数的唯一数据源（全部真实计数）
    let n_open = entries.iter().filter(|e| is_open_todo(e)).count();
    let n_done = entries.iter().filter(|e| is_done_entry(e)).count();
    let n_inbox = entries.iter().filter(|e| e.dir == "inbox").count();
    let n_ideas = entries.iter().filter(|e| e.dir == "ideas").count();
    let n_notes = entries.iter().filter(|e| e.dir == "notes").count();
    let n_archive = entries.iter().filter(|e| e.dir == "archive").count();
    let n_overdue = entries
        .iter()
        .filter(|e| {
            is_open_todo(e) && e.fm.due.as_deref().map(|d| d < today.as_str()).unwrap_or(false)
        })
        .count();
    // 六段之和恒等于全部条目数，中心数字与页头计数始终一致；逾期是未完成待办的子集，
    // 在图例里单独标注，不参与求和（避免重复计数）。
    let segs = [
        render::Seg { label: "收件", en: "INBOX", n: n_inbox, kind: 0, op: "0.72" },
        render::Seg { label: "待办·未完成", en: "TODO OPEN", n: n_open, kind: 1, op: "1" },
        render::Seg { label: "待办·已完成", en: "TODO DONE", n: n_done, kind: 0, op: "0.50" },
        render::Seg { label: "想法", en: "IDEAS", n: n_ideas, kind: 0, op: "0.38" },
        render::Seg { label: "笔记", en: "NOTES", n: n_notes, kind: 0, op: "0.28" },
        render::Seg { label: "归档", en: "ARCHIVE", n: n_archive, kind: 0, op: "0.20" },
    ];
    let instrument = format!("{}{}", render::ring(&segs), render::legend(&segs, n_overdue));

    // ---- 最近 14 天捕获：按文件名时间戳前缀逐日统计（SPEC §2 的前缀即本地创建时间）
    let now = Local::now();
    let mut days: Vec<(String, usize)> = Vec::new();
    for back in (0..14).rev() {
        let d = now - chrono::Duration::days(back);
        let key = d.format("%Y%m%d").to_string();
        let n = entries
            .iter()
            .filter(|e| e.dir != "archive" && e.stem.starts_with(&key))
            .count();
        days.push((d.format("%m-%d").to_string(), n));
    }

    // ---- 单条目详情页：舞台由单条档案主导，邻接导航按 created 序（数据已存在）
    for (i, e) in entries.iter().enumerate() {
        let root = render::root_of("");
        let mut fields = String::new();
        fields.push_str(&format!(
            "<div><span>类型 <span class=\"en\">TYPE</span></span><b>{}</b></div>",
            esc(&e.fm.type_)
        ));
        fields.push_str(&format!(
            "<div><span>目录 <span class=\"en\">FOLDER</span></span><b>{}</b></div>",
            esc(&e.dir)
        ));
        fields.push_str(&format!(
            "<div><span>创建 <span class=\"en\">CREATED</span></span><b>{}</b></div>",
            esc(&fmt_created(&e.fm.created))
        ));
        if e.fm.type_ == "todo" {
            fields.push_str(&format!(
                "<div><span>状态 <span class=\"en\">STATUS</span></span><b>{}</b></div>",
                esc(e.fm.status.as_deref().unwrap_or("open"))
            ));
        }
        if let Some(d) = &e.fm.due {
            fields.push_str(&format!(
                "<div><span>截止 <span class=\"en\">DUE</span></span><b>{}</b></div>",
                esc(d)
            ));
        }
        if let Some(d) = &e.fm.done {
            fields.push_str(&format!(
                "<div><span>完成 <span class=\"en\">DONE</span></span><b>{}</b></div>",
                esc(d)
            ));
        }
        if !e.fm.tags.is_empty() {
            let tl: Vec<String> = e
                .fm
                .tags
                .iter()
                .map(|t| {
                    format!(
                        "<a class=\"tag\" href=\"{root}tags.html#{}\">{}</a>",
                        esc(t),
                        esc(t)
                    )
                })
                .collect();
            fields.push_str(&format!(
                "<div><span>标签 <span class=\"en\">TAGS</span></span><b>{}</b></div>",
                tl.join(" ")
            ));
        }

        let mut actions = String::new();
        // archive 没有独立分类页，返回链接指向索引
        let back = if e.dir == "archive" {
            format!("{root}index.html")
        } else {
            format!("{root}{}.html", esc(&e.dir))
        };
        actions.push_str(&render::action(&back, "← 返回 <span class=\"en\">BACK</span>", "is-on-stage"));
        let mut adj = String::new();
        if i > 0 {
            adj.push_str(&render::entry_row(&entries[i - 1], root, true, &today));
            actions.push_str(&render::action(
                &format!("{root}pages/{}.html", esc(&entries[i - 1].stem)),
                "← 更新一条 <span class=\"en\">NEWER</span>",
                "is-on-stage",
            ));
        }
        if let Some(p) = entries.get(i + 1) {
            adj.push_str(&render::entry_row(p, root, true, &today));
            actions.push_str(&render::action(
                &format!("{root}pages/{}.html", esc(&p.stem)),
                "更早一条 <span class=\"en\">OLDER</span> →",
                "is-on-stage",
            ));
        }

        let mut meta = format!(
            "<span>创建</span> <span class=\"en\">CREATED</span> <b>{}</b>",
            esc(&fmt_created(&e.fm.created))
        );
        if e.fm.type_ == "todo" {
            meta.push_str(&format!(
                " · <span>状态</span> <span class=\"en\">STATUS</span> <b>{}</b>",
                esc(e.fm.status.as_deref().unwrap_or("open"))
            ));
            if let Some(d) = &e.fm.due {
                meta.push_str(&format!(
                    " · <span>截止</span> <span class=\"en\">DUE</span> <b>{}</b>",
                    esc(d)
                ));
            }
        }
        let stage = render::stage(
            &format!(
                "<span class=\"en\">{} / {}</span>",
                esc(&e.fm.type_),
                esc(&e.dir)
            ),
            &esc(&e.fm.title),
            &meta,
            "",
            &actions,
            "",
        );

        let mut body = format!("<div class=\"dossier\">{fields}</div>");
        body.push_str(&render::band(
            "",
            "01 / DOCUMENT",
            "正文",
            "",
            &format!("<div class=\"doc\">{}</div>", render_md(&e.body)),
            "reveal reveal-2",
        ));
        if !adj.is_empty() {
            body.push_str(&render::band(
                "",
                "02 / CHRONOLOGY",
                "邻接条目",
                "按创建时间",
                &format!("<ul class=\"rows\">{adj}</ul>"),
                "reveal reveal-3",
            ));
        }
        let page = render::Page::new(&e.fm.title, "").stage(stage).body(body);
        fs::write(pages.join(format!("{}.html", e.stem)), render::render(&page, &built, &count_line))
            .unwrap_or_else(|er| die(&format!("写入详情页失败: {er}")));
    }

    // ---- 全库检索索引 search.json（dashboard 搜索框 fetch 用，字段全 JSON 转义）
    let mut jb = String::from("{\"entries\":[");
    for (i, e) in entries.iter().enumerate() {
        if i > 0 {
            jb.push(',');
        }
        let tags_arr = e
            .fm
            .tags
            .iter()
            .map(|t| format!("\"{}\"", json_escape(t)))
            .collect::<Vec<_>>()
            .join(",");
        jb.push_str(&format!(
            "{{\"stem\":\"{}\",\"dir\":\"{}\",\"title\":\"{}\",\"tags\":[{}],\"created\":\"{}\",\"body\":\"{}\"}}",
            json_escape(&e.stem),
            json_escape(&e.dir),
            json_escape(&e.fm.title),
            tags_arr,
            json_escape(&e.fm.created),
            json_escape(&e.body),
        ));
    }
    jb.push_str("]}");
    fs::write(dist.join("search.json"), jb).unwrap();

    // ---- 标签索引：chip 索引 + 按 tag 分组的档案带（一个舞台里用规则行排，不是 N 个盒子）
    use std::collections::HashMap;
    let mut tag_map: HashMap<&str, Vec<&Entry>> = HashMap::new();
    for e in &entries {
        for t in &e.fm.tags {
            tag_map.entry(t.as_str()).or_default().push(e);
        }
    }
    let mut tag_names: Vec<&str> = tag_map.keys().copied().collect();
    tag_names.sort_by(|a, b| {
        tag_map[*b]
            .len()
            .cmp(&tag_map[*a].len())
            .then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
    });
    let mut tbody = String::new();
    if tag_names.is_empty() {
        tbody.push_str(&render::band(
            "",
            "00 / INDEX",
            "标签索引",
            "",
            "<p class=\"empty\">还没有标签。给条目打标签：mind new &lt;type&gt; \"标题\" --tags \"a,b\"</p>",
            "reveal reveal-2",
        ));
    } else {
        let mut chips = String::new();
        for t in &tag_names {
            chips.push_str(&format!(
                "<li><a class=\"chip\" href=\"#{}\">{}<b>{}</b></a></li>",
                esc(t),
                esc(t),
                tag_map[*t].len()
            ));
        }
        tbody.push_str(&render::band(
            "",
            "00 / INDEX",
            "标签索引",
            &format!("{} 个标签 / TAGS", tag_names.len()),
            &format!("<ul class=\"chips\">{chips}</ul>"),
            "reveal reveal-2",
        ));
        for (k, t) in tag_names.iter().enumerate() {
            let rows: String = tag_map[*t]
                .iter()
                .map(|e| render::entry_row(e, "", true, &today))
                .collect::<Vec<_>>()
                .join("\n");
            tbody.push_str(&render::band(
                &esc(t),
                &format!("{:02} / TAG", k + 1),
                &format!("#{}", esc(t)),
                &format!("<b>{}</b> 条", tag_map[*t].len()),
                &format!("<ul class=\"rows\">{rows}</ul>"),
                "",
            ));
        }
    }
    let tags_stage = render::stage(
        &render::kicker("档案索引", "ARCHIVE INDEX"),
        "TAGS",
        &format!(
            "<b>{}</b> 个标签 · <b>{total}</b> 条目",
            tag_names.len()
        ),
        "",
        "",
        "reveal",
    );
    let tags_page = render::Page::new("tags", "tags")
        .stage(tags_stage)
        .body(format!("<div class=\"bands\">{tbody}</div>"));
    fs::write(dist.join("tags.html"), render::render(&tags_page, &built, &count_line)).unwrap();

    // ---- 分类页（todo 页按 type 汇总全库，其余按目录）
    for dir in ["inbox", "todo", "ideas", "notes"] {
        let sub;
        let mut bands = String::new();
        if dir == "todo" {
            let mut open: Vec<&Entry> = entries.iter().filter(|e| is_open_todo(e)).collect();
            open.sort_by_key(|e| e.fm.due.clone().unwrap_or_else(|| "9999".into()));
            let open_rows: String = open
                .iter()
                .map(|e| render::entry_row(e, "", true, &today))
                .collect::<Vec<_>>()
                .join("\n");
            let open_body = if open.is_empty() {
                "<p class=\"empty\">没有未完成的待办。</p>".to_string()
            } else {
                format!("<ul class=\"rows\">{open_rows}</ul>")
            };
            bands.push_str(&render::band(
                "",
                "01 / OPEN",
                "未完成",
                &format!("<b>{}</b> 条 <span class=\"en\">OPEN</span>", open.len()),
                &open_body,
                "reveal reveal-2",
            ));

            let mut done: Vec<&Entry> = entries.iter().filter(|e| is_done_entry(e)).collect();
            done.sort_by_key(|e| std::cmp::Reverse(parse_created(&e.fm.created).unwrap_or(0)));
            if !done.is_empty() {
                let inner: String = done
                    .iter()
                    .map(|e| render::entry_row(e, "", true, &today))
                    .collect::<Vec<_>>()
                    .join("\n");
                let done_body = if done.len() > 8 {
                    format!(
                        "<div class=\"foldwrap\"><ul class=\"rows\" data-fold=\"8\">{inner}</ul>\
<button class=\"act foldbtn\" type=\"button\">展开全部 <span class=\"en\">SHOW ALL (+{})</span></button></div>",
                        done.len() - 8
                    )
                } else {
                    format!("<ul class=\"rows\">{inner}</ul>")
                };
                bands.push_str(&render::band(
                    "",
                    "02 / DONE",
                    "已完成",
                    &format!("<b>{}</b> 条 <span class=\"en\">DONE</span>", done.len()),
                    &done_body,
                    "reveal reveal-3",
                ));
            }
            sub = format!("<b>{}</b> 条未完成 · <b>{}</b> 条已完成", open.len(), done.len());
        } else {
            let list: Vec<&Entry> = entries.iter().filter(|e| e.dir == dir).collect();
            let rows: String = list
                .iter()
                .map(|e| render::entry_row(e, "", true, &today))
                .collect::<Vec<_>>()
                .join("\n");
            let body = if list.is_empty() {
                "<p class=\"empty\">这里还没有内容。可以这样写入：mind new idea \"标题\"</p>".to_string()
            } else {
                format!("<ul class=\"rows\">{rows}</ul>")
            };
            bands.push_str(&render::band(
                "",
                "01 / LIST",
                "全部条目",
                &format!("<b>{}</b> 条", list.len()),
                &body,
                "reveal reveal-2",
            ));
            sub = format!("<b>{}</b> 条", list.len());
        }
        let ident = match dir {
            "inbox" => "INBOX",
            "todo" => "TODOS",
            "ideas" => "IDEAS",
            _ => "NOTES",
        };
        let (kicker_cjk, kicker_en) = match dir {
            "inbox" => ("收件箱", "INBOX"),
            "todo" => ("待办", "TODOS"),
            "ideas" => ("想法", "IDEAS"),
            _ => ("笔记", "NOTES"),
        };
        let st = render::stage(&render::kicker(kicker_cjk, kicker_en), ident, &sub, "", "", "reveal");
        let page = render::Page::new(dir, dir)
            .stage(st)
            .body(format!("<div class=\"bands\">{bands}</div>"));
        fs::write(dist.join(format!("{dir}.html")), render::render(&page, &built, &count_line))
            .unwrap();
    }

    // ---- index dashboard
    let open_todos: Vec<&Entry> = entries.iter().filter(|e| is_open_todo(e)).collect();
    let n_more = open_todos.len().saturating_sub(12);

    // RECENT = 捕获流：排除归档回流与已完成 todo
    let recent: Vec<&Entry> = entries
        .iter()
        .filter(|e| e.dir != "archive" && !is_done_entry(e))
        .take(20)
        .collect();
    let recent_rows: String = recent
        .iter()
        .map(|e| render::entry_row(e, "", true, &today))
        .collect::<Vec<_>>()
        .join("\n");
    let recent_body = if recent.is_empty() {
        "<p class=\"empty\">库还是空的。试试：mind new idea \"第一个想法\"</p>".to_string()
    } else if recent.len() > 8 {
        format!(
            "<div class=\"foldwrap\"><ul class=\"rows\" data-fold=\"8\">{recent_rows}</ul>\
<button class=\"act foldbtn\" type=\"button\">展开全部 <span class=\"en\">SHOW ALL (+{})</span></button></div>",
            recent.len() - 8
        )
    } else {
        format!("<ul class=\"rows\">{recent_rows}</ul>")
    };
    // 左缘年表刻度与捕获流共享同一组 created 时间
    let recent_body = format!(
        "<div class=\"scale-col\">{}{recent_body}</div>",
        render::scale(&recent)
    );

    let mut todo_rows: Vec<String> = open_todos
        .iter()
        .take(12)
        .map(|e| render::entry_row(e, "", true, &today))
        .collect();
    if n_more > 0 {
        todo_rows.push(format!(
            "<li class=\"row\"><span class=\"row-date\">…</span><div class=\"row-main\">\
<a class=\"row-title\" href=\"todo.html\">还有 {} 条未完成</a></div><div class=\"row-side\"></div></li>",
            n_more
        ));
    }
    let todo_body = if todo_rows.is_empty() {
        "<p class=\"empty\">没有未完成的待办。</p>".to_string()
    } else {
        format!("<ul class=\"rows\">{}</ul>", todo_rows.join("\n"))
    };

    let search_body = "<div class=\"composer\">\
<label class=\"hint\" for=\"q\">关键词 KEYWORD</label>\
<input id=\"q\" class=\"q\" type=\"search\" placeholder=\"输入关键词 / TYPE TO SEARCH\" autocomplete=\"off\">\
<p class=\"hint\" id=\"qstatus\" role=\"status\">正在载入索引 / LOADING INDEX</p>\
<div id=\"results\"></div></div>"
        .to_string();

    let bands = format!(
        "<div class=\"bands is-split\">{}{}{}</div>",
        render::band("", "00 / SEARCH", "检索", "标题 · 标签 · 正文", &search_body, "reveal reveal-2 is-wide"),
        render::band(
            "",
            "01 / RECENT",
            "最近捕获",
            &format!("<b>{}</b> 条（显示最近 <b>{}</b> 条）", recent.len(), recent.len().min(8)),
            &recent_body,
            "reveal reveal-3",
        ),
        render::band(
            "",
            "02 / OPEN",
            "未完成待办",
            &format!("<b>{}</b> 条", open_todos.len()),
            &todo_body,
            "reveal reveal-4",
        ),
    );

    let index_stage = render::stage(
        &render::kicker("个人档案库", "PERSONAL ARCHIVE"),
        "ARCHIVE",
        &format!(
            "<span class=\"en\">SESSION</span> <b>{today}</b> · <span class=\"en\">LOCAL</span> \
<b><span id=\"clock\">--:--</span></b> · <b>{total}</b> <span class=\"en\">ENTRIES</span>"
        ),
        &format!("{instrument}{}", render::activity(&days)),
        "",
        "reveal",
    );
    let index_page = render::Page::new("dashboard", "index")
        .stage(index_stage)
        .body(bands);
    fs::write(dist.join("index.html"), render::render(&index_page, &built, &count_line)).unwrap();

    println!(
        "built {total} entries -> {}/dist (index + 4 pages + tags + {} detail pages + search.json)",
        vault.display(),
        entries.len()
    );
    if !errs.is_empty() {
        println!("注意: {} 个文件因格式问题未纳入视图，运行 mind check 查看详情", errs.len());
    }
}

// ---------------------------------------------------------------- serve

/// 最小 percent-decoding：浏览器会把空格等编码成 %XX，不做解码会导致构建出的文件 404
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn content_type(p: &Path) -> &'static str { match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "md" => "text/plain; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "ttf" => "font/ttf",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn cmd_serve(vault: PathBuf, port: u16, bind: String) {
    let dist = vault.join("dist");
    if !dist.is_dir() {
        eprintln!("{} 不存在，先运行 mind build", dist.display());
        exit(2);
    }
    let addr = format!("{bind}:{port}");
    let listener = std::net::TcpListener::bind(&addr)
        .unwrap_or_else(|e| die(&format!("监听 {addr} 失败: {e}")));
    // 探测本机局域网地址（UDP connect 不发包），给出可点击的 URL
    let host = std::net::UdpSocket::bind("0.0.0.0:0")
        .ok()
        .and_then(|s| {
            if s.connect("8.8.8.8:80").is_ok() {
                s.local_addr().ok()
            } else {
                None
            }
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    println!(
        "serving {}/dist at http://{host}:{port} (Ctrl+C 停止)",
        vault.display()
    );
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let dist = dist.clone();
        thread::spawn(move || {
            let mut buf = [0u8; 2048];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let Some(path) = req.split_whitespace().nth(1) else { return };
            let path = percent_decode(path.split('?').next().unwrap_or("/"));
            let rel = path.trim_start_matches('/');
            let mut target = dist.join(if rel.is_empty() { "index.html" } else { rel });
            if target.is_dir() {
                target = target.join("index.html");
            }
            let canonical = target.canonicalize().unwrap_or_else(|_| dist.clone());
            let dist_canon = dist.canonicalize().unwrap_or_else(|_| dist.clone());
            if !canonical.starts_with(&dist_canon) || !canonical.is_file() {
                let _ = stream.write_all(
                    b"HTTP/1.1 404 NOT FOUND\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<h1>404</h1>",
                );
                return;
            }
            match fs::read(&canonical) {
                Ok(data) => {
                    // dashboard 期望永远新鲜：mote build 后浏览器必须拿到新文件，
                    // 不发缓存头会触发浏览器启发式缓存，改版后"看起来没生效"
                    // 例外：ttf 字体体积大且文件名含字重、内容极少变更，允许缓存一天，
                    // 否则 8MB×每次翻页对局域网手机是实打实的负担
                    let cache = match canonical.extension().and_then(|e| e.to_str()) {
                        Some("ttf") | Some("woff2") => "public, max-age=86400",
                        _ => "no-store",
                    };
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: {}\r\n\r\n",
                        content_type(&canonical),
                        data.len(),
                        cache
                    );
                    let _ = stream.write_all(head.as_bytes());
                    let _ = stream.write_all(&data);
                }
                Err(_) => {
                    let _ = stream.write_all(b"HTTP/1.1 500 ERR\r\n\r\n");
                }
            }
        });
    }
}

fn die(msg: &str) -> ! {
    eprintln!("mind: {msg}");
    exit(1);
}
