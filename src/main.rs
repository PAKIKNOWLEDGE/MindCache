use std::env;
use std::fs;
use std::io::{Read, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::thread;

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, SecondsFormat, TimeZone};

const DIRS: [&str; 5] = ["inbox", "todo", "ideas", "notes", "archive"];
const TYPES: [&str; 4] = ["thought", "todo", "idea", "note"];
const VERSION: &str = env!("CARGO_PKG_VERSION");

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
                eprintln!("mind new <type> [title]  — type: idea|todo|note（--inbox 放入 inbox/）");
                exit(2);
            }
            let type_ = pos[0].to_string();
            let title = if pos.len() > 1 {
                pos[1..].iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ")
            } else {
                String::new()
            };
            cmd_new(&vault, &type_, &title, inbox);
        }
        "check" => cmd_check(extract_vault(&args[1..]), positional_path(&args[1..])),
        "build" => cmd_build(extract_vault(&args[1..])),
        "serve" => {
            let vault = extract_vault(&args[1..]);
            let port = extract_port(&args[1..]);
            cmd_serve(vault, port);
        }
        "path" => println!("{}", extract_vault(&args[1..]).display()),
        "help" | "--help" | "-h" => usage(),
        _ => usage(),
    }
}

/// 带值的 flag 名；positionals 跳过它们及其后的值 token
const FLAG_WITH_VALUE: [&str; 4] = ["--vault", "-v", "--port", "-p"];

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
  mind check [FILE.md]          lint vault entries (or a single file)
  mind build                    generate static dashboard into <vault>/dist/
  mind serve [--port N]         serve <vault>/dist/ over LAN (default port 8181)
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
        let v = v.trim().trim_matches('"').trim_matches('\'').to_string();
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
                    fm.tags = v[1..]
                        .strip_suffix(']')
                        .unwrap_or(&v[1..])
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
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

fn cmd_new(vault: &Path, type_: &str, title: &str, inbox: bool) {
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
    let fm = if type_ == "todo" {
        format!(
            "---\ntype: todo\ntitle: {title}\ncreated: {created}\nstatus: open\ntags: []\n---\n\n"
        )
    } else {
        format!(
            "---\ntype: {type_}\ntitle: {title}\ncreated: {created}\ntags: []\n---\n\n"
        )
    };
    fs::write(&path, fm).unwrap_or_else(|e| die(&format!("写入失败: {e}")));
    println!("{}", path.display());
}

// ---------------------------------------------------------------- check

fn cmd_check(vault: PathBuf, single: Option<PathBuf>) {
    let mut errs: Vec<(String, String)> = Vec::new();
    let mut ok = 0usize;

    let check_one = |rel: String, text: String, dir: String, stem: String,
                     errs: &mut Vec<(String, String)>, ok: &mut usize| {
        let mut es: Vec<String> = Vec::new();
        if !valid_filename(&stem) {
            es.push("文件名不符合 YYYYMMDD-HHMM-ascii-slug 格式（仅限 ASCII）".into());
        }
        match parse_fm(&text) {
            Ok(fm) => {
                if !TYPES.contains(&fm.type_.as_str()) {
                    es.push(format!("type 无效: \"{}\"", fm.type_));
                }
                if fm.title.trim().is_empty() {
                    es.push("title 缺失或为空".into());
                }
                if fm.created.is_empty() {
                    es.push("created 缺失".into());
                } else if parse_created(&fm.created).is_none() {
                    es.push(format!("created 无法解析: \"{}\"", fm.created));
                }
                if fm.type_ == "todo" {
                    if let Some(st) = &fm.status {
                        if st != "open" && st != "done" {
                            es.push(format!("status 无效: \"{st}\"（应为 open|done）"));
                        }
                    }
                    if let Some(due) = &fm.due {
                        if NaiveDate::parse_from_str(due, "%Y-%m-%d").is_err() {
                            es.push(format!("due 无法解析: \"{due}\"（应为 YYYY-MM-DD）"));
                        }
                    }
                    if let Some(d) = &fm.done {
                        if NaiveDate::parse_from_str(d, "%Y-%m-%d").is_err() {
                            es.push(format!("done 无法解析: \"{d}\"（应为 YYYY-MM-DD）"));
                        }
                    }
                }
                // type 与目录一致性：映射目录 / inbox / archive 皆合法
                if let Some(expect) = type_dir(&fm.type_) {
                    if dir != expect && dir != "inbox" && dir != "archive" {
                        es.push(format!("type {} 一般放 {expect}/，当前在 {dir}/", fm.type_));
                    }
                }
            }
            Err(e) => es.push(e),
        }
        if es.is_empty() {
            *ok += 1;
        } else {
            for e in es {
                errs.push((rel.clone(), e));
            }
        }
    };

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
                Ok(text) => check_one(name, text, dir, stem, &mut errs, &mut ok),
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
            errs.extend(parse_errs);
            ok += entries.len();
            // 对已解析条目做剩余校验
            for e in &entries {
                let rel = format!("{}/{}.md", e.dir, e.stem);
                let mut es: Vec<String> = Vec::new();
                if !valid_filename(&e.stem) {
                    es.push("文件名不符合 YYYYMMDD-HHMM-ascii-slug 格式（仅限 ASCII）".into());
                }
                if !TYPES.contains(&e.fm.type_.as_str()) {
                    es.push(format!("type 无效: \"{}\"", e.fm.type_));
                }
                if e.fm.title.trim().is_empty() {
                    es.push("title 缺失或为空".into());
                }
                if e.fm.created.is_empty() || parse_created(&e.fm.created).is_none() {
                    es.push(format!("created 无法解析: \"{}\"", e.fm.created));
                }
                if e.fm.type_ == "todo" {
                    if let Some(st) = &e.fm.status {
                        if st != "open" && st != "done" {
                            es.push(format!("status 无效: \"{st}\""));
                        }
                    }
                    if let Some(due) = &e.fm.due {
                        if NaiveDate::parse_from_str(due, "%Y-%m-%d").is_err() {
                            es.push(format!("due 无法解析: \"{due}\""));
                        }
                    }
                    if let Some(d) = &e.fm.done {
                        if NaiveDate::parse_from_str(d, "%Y-%m-%d").is_err() {
                            es.push(format!("done 无法解析: \"{d}\""));
                        }
                    }
                }
                if let Some(expect) = type_dir(&e.fm.type_) {
                    if e.dir != expect && e.dir != "inbox" && e.dir != "archive" {
                        es.push(format!("type {} 一般放 {expect}/，当前在 {}/", e.fm.type_, e.dir));
                    }
                }
                if es.is_empty() {
                    // 已计入 ok
                } else {
                    ok -= 1; // 从通过数中扣除
                    for m in es {
                        errs.push((rel.clone(), m));
                    }
                }
            }
        }
    }

    for (f, e) in &errs {
        println!("ERR {f}: {e}");
    }
    println!(
        "checked: {ok} ok, {} error(s), vault: {}",
        errs.len(),
        vault.display()
    );
    if !errs.is_empty() {
        exit(1);
    }
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

// 主题三态：默认跟随系统；data-theme 覆盖。concat! 只接受字面量，
// light/dark 两组 token 各出现两份（:root 基准 + 显式覆盖），改色值时四处同步。
const CSS: &str = concat!(
    r###":root{ --paper:#f2ecdf; --panel:#f7f2e7; --card:#efe7d6; --ink:#2b2620; --muted:#756a58; --line:#d3c8b1; --accent:#b3502a; --shadow:rgba(80,60,30,.08); }
:root[data-theme="light"]{ --paper:#f2ecdf; --panel:#f7f2e7; --card:#efe7d6; --ink:#2b2620; --muted:#756a58; --line:#d3c8b1; --accent:#b3502a; --shadow:rgba(80,60,30,.08); ;color-scheme:light }
@media (prefers-color-scheme: dark){ :root:not([data-theme="light"]){ --paper:#15120e; --panel:#1c1812; --card:#211c15; --ink:#e8e0d0; --muted:#968b77; --line:#403728; --accent:#d96a3b; --shadow:rgba(0,0,0,.4); ;color-scheme:dark } }
:root[data-theme="dark"]{ --paper:#15120e; --panel:#1c1812; --card:#211c15; --ink:#e8e0d0; --muted:#968b77; --line:#403728; --accent:#d96a3b; --shadow:rgba(0,0,0,.4); ;color-scheme:dark }"###,
    r###"*{box-sizing:border-box}
html,body{margin:0;padding:0}
body{background:var(--paper);color:var(--ink);
  font-family:ui-monospace,"Cascadia Mono","SF Mono",Consolas,Menlo,monospace;
  font-size:14px;line-height:1.65;}
a{color:inherit;text-decoration:none}
a:hover{color:var(--accent)}
.serif{font-family:Georgia,"Times New Roman","Noto Serif CJK SC","Noto Serif SC","Source Han Serif SC","Songti SC",STSong,SimSun,serif}
.wrap{max-width:1180px;margin:0 auto;padding:18px 16px 40px}
.topbar{display:flex;justify-content:space-between;gap:12px;flex-wrap:wrap;
  border:1px solid var(--line);padding:8px 14px;font-size:11px;
  letter-spacing:.12em;text-transform:uppercase;color:var(--muted)}
.topbar b{color:var(--accent);font-weight:400}
nav{display:flex;gap:2px;margin-top:10px;flex-wrap:wrap}
nav a{border:1px solid var(--line);border-bottom:none;padding:6px 14px;font-size:11px;
  letter-spacing:.12em;text-transform:uppercase;color:var(--muted);background:var(--panel)}
nav a.on{color:var(--accent)}
.tbtn{background:none;border:1px solid var(--line);color:var(--muted);font:inherit;font-size:11px;letter-spacing:.12em;padding:2px 8px;cursor:pointer;text-transform:uppercase}
.tbtn:hover{color:var(--accent);border-color:var(--accent)}
nav a:hover{color:var(--accent)}
.label{font-size:11px;letter-spacing:.14em;text-transform:uppercase;color:var(--muted);
  border-bottom:1px solid var(--line);padding-bottom:8px;margin-bottom:12px}
.label b{color:var(--accent);font-weight:400}
.panel{border:1px solid var(--line);background:var(--panel);padding:16px 18px;box-shadow:0 1px 0 var(--shadow)}
.grid{display:grid;grid-template-columns:250px 1fr 300px;gap:14px;margin-top:14px;align-items:start}
@media(max-width:920px){.grid{grid-template-columns:1fr}
.grid>*{order:0}
.grid>*:nth-child(1){order:3}
.grid>*:nth-child(3){order:2}}
.hero{display:flex;justify-content:space-between;align-items:baseline;gap:16px;flex-wrap:wrap;margin-top:14px}
.clock{font-size:34px;letter-spacing:.08em;color:var(--ink)}
.clock small{font-size:12px;color:var(--muted);letter-spacing:.14em;display:block;text-align:right}
.stat{display:flex;justify-content:space-between;padding:5px 0;font-size:13px}
.stat a{color:var(--ink)} .stat a:hover{color:var(--accent)}
.stat .n{color:var(--accent)}
.dot{display:inline-block;width:7px;height:7px;background:var(--accent);margin-right:8px;vertical-align:1px}
.row{display:flex;justify-content:space-between;align-items:baseline;gap:10px;padding:8px 0;border-bottom:1px solid var(--line)}
.row:last-child{border-bottom:none}
.row>div{min-width:0}
.row .t{font-size:16px;overflow-wrap:anywhere}
.row .m{font-size:11px;color:var(--muted);white-space:nowrap;letter-spacing:.06em}
.row .fold{color:var(--accent);font-size:11px;letter-spacing:.1em;text-transform:uppercase}
.tag{font-size:11px;color:var(--muted);margin-right:6px}
.tag::before{content:"#"}
.due{font-size:11px;color:var(--muted);white-space:nowrap}
.row>*:last-child{flex-shrink:0}
.overdue{color:var(--accent)}
.done .t{text-decoration:line-through;color:var(--muted)}
.done .fold,.done .date,.done .tag,.done .m{color:var(--muted)}
.vaultwrap{display:flex;flex-direction:column;gap:14px}
.date{font-size:11px;color:var(--muted);margin-right:6px}
.empty{color:var(--muted);font-style:italic;padding:10px 0}
.statusbar{margin-top:16px;border:1px solid var(--line);padding:7px 14px;font-size:11px;
  letter-spacing:.12em;text-transform:uppercase;color:var(--muted);
  display:flex;justify-content:space-between;gap:10px;flex-wrap:wrap}
/* entry page */
.entry-meta{font-size:11px;letter-spacing:.14em;text-transform:uppercase;color:var(--muted);margin:14px 0 4px}
.entry-meta b{color:var(--accent);font-weight:400}
h1.entry{font-size:34px;font-weight:400;margin:6px 0 18px;line-height:1.3}
.back{display:inline-block;margin-bottom:10px;font-size:11px;letter-spacing:.12em;text-transform:uppercase;color:var(--muted)}
.back:hover{color:var(--accent)}
.body{max-width:70ch;font-size:15px}
.body a{text-decoration:underline;text-decoration-color:var(--line);text-underline-offset:3px}
.body a:hover{color:var(--accent)}
.body p{margin:.7em 0}
.body h1,.body h2,.body h3{font-family:Georgia,"Noto Serif CJK SC","Noto Serif SC",serif;font-weight:400;line-height:1.4}
.body h1{font-size:24px} .body h2{font-size:20px} .body h3{font-size:17px}
.body code{background:var(--card);border:1px solid var(--line);padding:0 5px;font-size:.9em}
.body pre{background:var(--card);border:1px solid var(--line);padding:12px 14px;overflow-x:auto;font-size:13px}
.body pre code{border:none;padding:0;background:none}
.body blockquote{margin:.8em 0;padding:.2em 1em;border-left:3px solid var(--accent);color:var(--muted);font-style:italic}
.body ul,.body ol{padding-left:1.4em}
.body table{border-collapse:collapse;margin:.8em 0}
.body th,.body td{border:1px solid var(--line);padding:4px 12px;font-size:13px;text-align:left}
.body th{background:var(--card);font-weight:400}
.body hr{border:none;border-top:1px solid var(--line);margin:1.4em 0}
.body img{max-width:100%}
"###);

fn page_html(title: &str, nav_active: &str, body: &str, built: &str, count_line: &str) -> String {
    let title = esc(title); // <title> 不转义会破坏文档头/注入脚本
    // 只有详情页位于 pages/ 子目录（nav_active 为空），根级链接才需要 ../ 前缀
    let root = if nav_active.is_empty() { "../" } else { "" };
    let nav = [
        ("index.html", "INDEX", "index"),
        ("inbox.html", "INBOX", "inbox"),
        ("todo.html", "TODO", "todo"),
        ("ideas.html", "IDEAS", "ideas"),
        ("notes.html", "NOTES", "notes"),
    ]
    .iter()
    .map(|(href, name, key)| {
        let class = if *key == nav_active { " class=\"on\"" } else { "" };
        format!("<a href=\"{root}{href}\"{class}>{name}</a>")
    })
    .collect::<Vec<_>>()
    .join("");
    format!(
        "<!DOCTYPE html>\n<html lang=\"zh\">\n<head>\n<meta charset=\"utf-8\">\n<script>(function(){{try{{var t=localStorage.getItem(\"mind-theme\");if(t)document.documentElement.setAttribute(\"data-theme\",t)}}catch(e){{}}}})();</script>\n\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n\
<title>{title} · MIND</title>\n<meta name=\"color-scheme\" content=\"light dark\">\n<meta name=\"theme-color\" content=\"#f2ecdf\" media=\"(prefers-color-scheme: light)\">\n<meta name=\"theme-color\" content=\"#15120e\" media=\"(prefers-color-scheme: dark)\">\n<link rel=\"icon\" href=\"data:,\">\n<link rel=\"stylesheet\" href=\"{root}style.css\">\n</head>\n<body>\n\
<div class=\"wrap\">\n\
<div class=\"topbar\"><span><b>MIND</b> // PERSONAL KNOWLEDGE BASE</span><span>{count_line} &nbsp;<button class=\"tbtn\" id=\"themebtn\">THEME</button></span></div>\n\
<nav>{nav}</nav>\n\
{body}\n\
<div class=\"statusbar\"><span>MIND v{VERSION}</span><span>{built}</span></div>\n\
</div>\n<script>\n(function(){{var c=document.getElementById('clock');if(c){{function t(){{var d=new Date();c.childNodes[0].nodeValue=('0'+d.getHours()).slice(-2)+':'+('0'+d.getMinutes()).slice(-2);}}t();setInterval(t,60000);}}\n}})();\n
var b=document.getElementById('themebtn');
if(b){{
  var modes=['auto','dark','light'];
  function mcur(){{try{{return localStorage.getItem('mind-theme')||'auto'}}catch(e){{return 'auto'}}}}
  function mpaint(){{b.textContent='THEME: '+mcur().toUpperCase()}}
  b.addEventListener('click',function(){{
    var nx=modes[(modes.indexOf(mcur())+1)%3];
    if(nx==='auto'){{try{{localStorage.removeItem('mind-theme')}}catch(e){{}}document.documentElement.removeAttribute('data-theme')}}
    else{{try{{localStorage.setItem('mind-theme',nx)}}catch(e){{}}document.documentElement.setAttribute('data-theme',nx)}}
    mpaint();
  }});
  mpaint();
}}\n</script>\n</body>\n</html>\n",
    )
}

fn is_done_entry(e: &Entry) -> bool {
    e.fm.type_ == "todo" && e.fm.status.as_deref() == Some("done")
}

fn is_open_todo(e: &&Entry) -> bool {
    e.fm.type_ == "todo" && e.fm.status.as_deref().unwrap_or("open") != "done"
}

fn entry_row(e: &Entry, rel_prefix: &str, show_dir: bool) -> String {
    let fold = if show_dir {
        format!("<span class=\"fold\">{}</span> ", esc(&e.dir))
    } else {
        String::new()
    };
    let tags = e
        .fm
        .tags
        .iter()
        .map(|t| format!("<span class=\"tag\">{}</span>", esc(t)))
        .collect::<Vec<_>>()
        .join("");
    let row_class = if is_done_entry(e) { "row done" } else { "row" };
    let m = tags_for_meta(e);
    let m_html = if m.is_empty() {
        String::new()
    } else {
        format!("<span class=\"m\">{m}</span>")
    };
    format!(
        "<div class=\"{row_class}\"><div><span class=\"date\">{date}</span> {fold}<a class=\"t serif\" href=\"{prefix}pages/{stem}.html\">{title}</a> {tags}</div>{m_html}</div>",
        date = esc(&fmt_created(&e.fm.created)),
        stem = esc(&e.stem),
        title = esc(&e.fm.title),
        prefix = rel_prefix,
    )
}

/// 共享的 due 渲染：overdue 高亮在所有视图一致
fn due_span(e: &Entry) -> String {
    e.fm.due
        .as_deref()
        .map(|d| {
            let overdue = d < &Local::now().format("%Y-%m-%d").to_string();
            format!(
                "<span class=\"due{}\">due {}</span>",
                if overdue { " overdue" } else { "" },
                esc(d)
            )
        })
        .unwrap_or_default()
}

fn tags_for_meta(e: &Entry) -> String {
    if e.fm.type_ == "todo" {
        let s = esc(e.fm.status.as_deref().unwrap_or("open"));
        let due = due_span(e);
        if due.is_empty() {
            s
        } else {
            format!("{s} {due}")
        }
    } else {
        String::new()
    }
}

fn sort_by_created(entries: &mut [Entry]) {
    entries.sort_by_key(|e| std::cmp::Reverse(parse_created(&e.fm.created).unwrap_or(0)));
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

    let dist = vault.join("dist");
    let pages = dist.join("pages");
    let _ = fs::remove_dir_all(&pages);
    fs::create_dir_all(&pages).unwrap_or_else(|e| die(&format!("创建 dist 失败: {e}")));
    fs::write(dist.join("style.css"), CSS).unwrap();

    sort_by_created(&mut entries);

    let total = entries.len();
    let count_line = format!("{} ENTRIES", total);

    // ---- 单条目详情页
    for e in &entries {
        let tags = e
            .fm
            .tags
            .iter()
            .map(|t| format!("<span class=\"tag\">{}</span>", esc(t)))
            .collect::<Vec<_>>()
            .join("");
        let mut meta = format!(
            "<b>{}</b> // {} // {}",
            esc(&e.fm.type_),
            esc(&e.dir),
            esc(&fmt_created(&e.fm.created))
        );
        if e.fm.type_ == "todo" {
            meta.push_str(&format!(
                " // {}",
                esc(e.fm.status.as_deref().unwrap_or("open"))
            ));
            if let Some(d) = &e.fm.due {
                meta.push_str(&format!(" <span class=\"due\">due {}</span>", esc(d)));
            }
        }
        // archive 没有独立分类页，返回链接指向 dashboard
        let back = if e.dir == "archive" {
            "../index.html".to_string()
        } else {
            format!("../{}.html", esc(&e.dir))
        };
        let body = format!(
            "<a class=\"back\" href=\"{back}\">← BACK</a>\n\
<div class=\"entry-meta\">{meta}</div>\n\
<h1 class=\"entry serif\">{title}</h1>\n\
<div>{tags}</div>\n\
<div class=\"body\">{md}</div>",
            meta = meta,
            title = esc(&e.fm.title),
            md = render_md(&e.body),
        );
        let html = page_html(&e.fm.title, "", &body, &built, &count_line);
        fs::write(pages.join(format!("{}.html", e.stem)), html)
            .unwrap_or_else(|er| die(&format!("写入详情页失败: {er}")));
    }

    // ---- 分类页（todo 页按 type 汇总全库，其余按目录）
    for dir in ["inbox", "todo", "ideas", "notes"] {
        let (rows, sub) = if dir == "todo" {
            let mut open: Vec<&Entry> =
                entries.iter().filter(|e| is_open_todo(e)).collect();
            open.sort_by_key(|e| e.fm.due.clone().unwrap_or_else(|| "9999".into()));
            let mut done: Vec<&Entry> = entries.iter().filter(|e| is_done_entry(e)).collect();
            done.sort_by_key(|e| std::cmp::Reverse(parse_created(&e.fm.created).unwrap_or(0)));
            let n_done_total = done.len();
            let mut rows: Vec<String> = open
                .iter()
                .map(|e| entry_row(e, "", false))
                .collect();
            if rows.is_empty() && done.is_empty() {
                rows.push("<div class=\"empty\">no todos.</div>".into());
            }
            for e in done.iter().take(10) {
                rows.push(entry_row(e, "", false));
            }
            if n_done_total > 10 {
                rows.push(format!(
                    "<div class=\"row\"><span class=\"m\">… {} more done entries</span></div>",
                    n_done_total - 10
                ));
            }
            let sub = format!(" // <b>{}</b> OPEN", open.len());
            (rows.join("\n"), sub)
        } else {
            let list: Vec<&Entry> = entries.iter().filter(|e| e.dir == dir).collect();
            let rows = if list.is_empty() {
                "<div class=\"empty\">nothing here yet.</div>".to_string()
            } else {
                list.iter().map(|e| entry_row(e, "", false)).collect::<Vec<_>>().join("\n")
            };
            (rows, String::new())
        };
        let body = format!(
            "<div class=\"panel\" style=\"margin-top:14px\">\n<div class=\"label\"><b>01</b> // {}{sub}</div>\n{rows}\n</div>",
            dir.to_uppercase(),
        );
        let html = page_html(dir, dir, &body, &built, &count_line);
        fs::write(dist.join(format!("{dir}.html")), html).unwrap();
    }

    // ---- index dashboard
    let open_todos: Vec<&Entry> = entries.iter().filter(is_open_todo).collect();
    let n_todo_open = open_todos.len();
    let n_inbox = entries.iter().filter(|e| e.dir == "inbox").count();
    let n_ideas = entries.iter().filter(|e| e.dir == "ideas").count();
    let n_notes = entries.iter().filter(|e| e.dir == "notes").count();
    let n_archive = entries.iter().filter(|e| e.dir == "archive").count();

    // RECENT = 捕获流：排除归档回流与已完成 todo
    let recent: Vec<&Entry> = entries
        .iter()
        .filter(|e| e.dir != "archive" && !is_done_entry(e))
        .take(20)
        .collect();
    let recent_rows = if recent.is_empty() {
        "<div class=\"empty\">vault is empty — run: mind new idea \"hello\"</div>".to_string()
    } else {
        recent
            .iter()
            .map(|e| entry_row(e, "", true))
            .collect::<Vec<_>>()
            .join("\n")
    };
    // OPEN TODOS 按 due 升序（overdue 优先），无 due 靠后；超出 12 条折叠
    let mut sorted_todos: Vec<&&Entry> = open_todos.iter().collect();
    sorted_todos.sort_by_key(|e| e.fm.due.clone().unwrap_or_else(|| "9999".into()));
    let n_more = sorted_todos.len().saturating_sub(12);
    let mut todo_parts: Vec<String> = sorted_todos
        .iter()
        .take(12)
        .map(|e| {
            format!(
                "<div class=\"row\"><div><span class=\"dot\"></span><a class=\"t serif\" href=\"pages/{}.html\">{}</a></div>{}</div>",
                esc(&e.stem),
                esc(&e.fm.title),
                due_span(e)
            )
        })
        .collect();
    if n_more > 0 {
        todo_parts.push(format!(
            "<div class=\"row\"><a href=\"todo.html\"><span class=\"m\">… {} more open todos</span></a></div>",
            n_more
        ));
    }
    let todo_rows = if todo_parts.is_empty() {
        "<div class=\"empty\">no open todos.</div>".to_string()
    } else {
        todo_parts.join("\n")
    };
    let today = Local::now().format("%Y-%m-%d").to_string();
    let index_body = format!(
        "<div class=\"panel hero\"><div><div class=\"label\"><b>01</b> // SESSION // {today}</div></div>\n\
<div class=\"clock\"><span id=\"clock\">--:--</span><small>LOCAL TIME</small></div></div>\n\
<div class=\"grid\">\n\
<div class=\"vaultwrap\">\n\
  <div class=\"panel\"><div class=\"label\"><b>02</b> // VAULT</div>\n\
    <div class=\"stat\"><span>INBOX</span><span class=\"n\"><a href=\"inbox.html\">{n_inbox}</a></span></div>\n\
    <div class=\"stat\"><span>TODO · OPEN</span><span class=\"n\"><a href=\"todo.html\">{n_todo_open}</a></span></div>\n\
    <div class=\"stat\"><span>IDEAS</span><span class=\"n\"><a href=\"ideas.html\">{n_ideas}</a></span></div>\n\
    <div class=\"stat\"><span>NOTES</span><span class=\"n\"><a href=\"notes.html\">{n_notes}</a></span></div>\n\
    <div class=\"stat\"><span>ARCHIVE</span><span class=\"n\">{n_archive}</span></div>\n\
  </div>\n\
</div>\n\
<div class=\"panel\"><div class=\"label\"><b>03</b> // RECENT CAPTURES</div>\n{recent_rows}\n</div>\n\
<div class=\"panel\"><div class=\"label\"><b>04</b> // OPEN TODOS</div>\n{todo_rows}\n</div>\n\
</div>",
    );
    let html = page_html("dashboard", "index", &index_body, &built, &count_line);
    fs::write(dist.join("index.html"), html).unwrap();

    println!(
        "built {} entries -> {}/dist (index + 4 pages + {} detail pages)",
        total,
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

fn content_type(p: &Path) -> &'static str {    match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
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
        _ => "application/octet-stream",
    }
}

fn cmd_serve(vault: PathBuf, port: u16) {
    let dist = vault.join("dist");
    if !dist.is_dir() {
        eprintln!("{} 不存在，先运行 mind build", dist.display());
        exit(2);
    }
    let addr = format!("0.0.0.0:{port}");
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
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\n\r\n",
                        content_type(&canonical),
                        data.len()
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
