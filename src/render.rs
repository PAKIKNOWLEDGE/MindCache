// ---------------------------------------------------------------- 渲染层
// ark-ui 契约：family = exa（固定）、depth = moderate、外观轴 auto/light/dark。
// 样式在 assets/ark/*.css（appearance / family-exa / depth-moderate / base），
// 本文件只负责把它们与真实数据组装成页面。任何文字容器都不得 overflow:hidden。

use crate::{esc, fmt_created, parse_created, Entry};

pub const CSS: &str = concat!(
    include_str!("../assets/ark/appearance.css"),
    include_str!("../assets/ark/family-exa.css"),
    include_str!("../assets/ark/depth-moderate.css"),
    include_str!("../assets/ark/base.css"),
);

/// 详情页位于 pages/ 子目录，其余页面在根级
/// 外壳词标（产品全名；换写法只改这一处）
pub const MARK: &str = "MindCache";

pub fn root_of(nav_active: &str) -> &'static str {
    if nav_active.is_empty() { "../" } else { "" }
}

const NAV: [(&str, &str, &str); 6] = [
    ("index.html", "01", "索引"),
    ("inbox.html", "02", "收件"),
    ("todo.html", "03", "待办"),
    ("ideas.html", "04", "想法"),
    ("notes.html", "05", "笔记"),
    ("tags.html", "06", "标签"),
];

/// 外观与布局的启动脚本：必须在首次绘制前跑，避免闪烁；顺带做旧键一次性迁移
const BOOT: &str = r#"(function(){try{
var d=document.documentElement;
var a=localStorage.getItem('mind-appearance');
if(!a){var l=localStorage.getItem('mind-theme');
 if(l){a=(l==='light')?'light':(l==='dark'||l==='endfield')?'dark':'auto';
  try{localStorage.setItem('mind-appearance',a);localStorage.removeItem('mind-theme')}catch(e){}}}
if(a&&a!=='auto')d.setAttribute('data-ark-appearance',a);
var v=localStorage.getItem('mind-layout');
if(!v){var ld=localStorage.getItem('mind-density');
 if(ld){v=(ld==='2')?'list':'grid';try{localStorage.setItem('mind-layout',v);localStorage.removeItem('mind-density')}catch(e){}}}
if(v==='grid')d.setAttribute('data-ark-layout','grid');
}catch(e){}})();"#;

const JS: &str = r#"(function(){
var d=document.documentElement;
var c=document.getElementById('clock');
if(c){var tick=function(){var n=new Date();c.textContent=('0'+n.getHours()).slice(-2)+':'+('0'+n.getMinutes()).slice(-2)};tick();setInterval(tick,60000)}

function aget(){try{return localStorage.getItem('mind-appearance')||'auto'}catch(e){return 'auto'}}
var ab=document.getElementById('appearancebtn');
function apaint(){if(ab)ab.textContent='外观 THEME · '+({auto:'AUTO',light:'LIGHT',dark:'DARK'})[aget()]}
if(ab){ab.addEventListener('click',function(){
  var order=['auto','light','dark'],nx=order[(order.indexOf(aget())+1)%order.length];
  try{localStorage.setItem('mind-appearance',nx)}catch(e){}
  if(nx==='auto')d.removeAttribute('data-ark-appearance');else d.setAttribute('data-ark-appearance',nx);
  apaint()});apaint()}

function lget(){try{return localStorage.getItem('mind-layout')||'list'}catch(e){return 'list'}}
var lb=document.getElementById('layoutbtn');
function lpaint(){if(lb)lb.textContent='视图 VIEW · '+(lget()==='grid'?'GRID':'LIST')}
if(lb){lb.addEventListener('click',function(){
  var nx=lget()==='grid'?'list':'grid';
  try{localStorage.setItem('mind-layout',nx)}catch(e){}
  d.setAttribute('data-ark-layout',nx);lpaint()});lpaint()}

var fw=document.querySelectorAll('.rows[data-fold]');
for(var i=0;i<fw.length;i++){(function(w){
  var n=parseInt(w.getAttribute('data-fold'),10)||8;
  var rows=w.querySelectorAll(':scope > .row');
  if(rows.length<=n)return;
  for(var j=n;j<rows.length;j++)rows[j].classList.add('hidden-row');
  var btn=w.parentElement?w.parentElement.querySelector('.foldbtn'):null;
  if(!btn)return;
  btn.addEventListener('click',function(){
    for(var j=0;j<rows.length;j++)rows[j].classList.remove('hidden-row');
    if(btn.parentElement)btn.parentElement.removeChild(btn);
  });
})(fw[i])}

// 一次性彩蛋：首次访问把舞台词标闪成 M1NDC4CH3（9 字符等宽，切换不产生位移），
// 约 0.8s 后回到 MindCache；reduced-motion 或已访问过则完全跳过。
try{
  var mk=document.querySelector('.stage-kicker .mark');
  var seen=null;try{seen=localStorage.getItem('mind-seen')}catch(e){}
  var rm=false;try{rm=window.matchMedia('(prefers-reduced-motion: reduce)').matches}catch(e){}
  if(mk&&!seen&&!rm){
    var keep=mk.textContent;
    mk.textContent='M1NDC4CH3';mk.classList.add('is-leet');
    setTimeout(function(){mk.textContent=keep;mk.classList.remove('is-leet')},800);
  }
  if(!seen){try{localStorage.setItem('mind-seen','1')}catch(e){}}
}catch(e){}

var q=document.getElementById('q');
if(q){
  var idx=null,res=document.getElementById('results'),st=document.getElementById('qstatus');
  function say(t){if(st)st.textContent=t}
  var band=q.closest('.band');
  function drop(){if(band&&band.parentElement)band.parentElement.removeChild(band)}
  function render(hits){
    if(!res)return;
    res.textContent='';
    var ul=document.createElement('ul');ul.className='rows';
    for(var i=0;i<hits.length;i++){
      var e=hits[i],li=document.createElement('li');li.className='row';
      var dt=document.createElement('span');dt.className='row-date';dt.textContent=(e.created||'').slice(0,10);
      var main=document.createElement('div');main.className='row-main';
      var a=document.createElement('a');a.className='row-title';a.href='pages/'+e.stem+'.html';a.textContent=e.title;
      var kind=document.createElement('span');kind.className='row-kind';kind.textContent=e.dir||'';
      main.appendChild(a);main.appendChild(kind);
      var facets=document.createElement('span');facets.className='row-facets';
      var tg=e.tags||[];
      for(var k=0;k<tg.length;k++){
        var t=document.createElement('span');t.className='tag';t.textContent=tg[k];facets.appendChild(t);
      }
      if(tg.length)main.appendChild(facets);
      li.appendChild(dt);li.appendChild(main);ul.appendChild(li);
    }
    res.appendChild(ul);
  }
  // 无 fetch（file:// 或老浏览器）时隐藏检索面板，而不是留一个坏掉的控件
  if(typeof fetch!=='function'){drop();return}
  fetch('search.json').then(function(r){return r.json()}).then(function(dd){
    idx=dd.entries||[];say('索引就绪 / INDEX READY · '+idx.length+' 条');
    if(q.value)q.dispatchEvent(new Event('input'));
  }).catch(drop);
  q.addEventListener('input',function(){
    if(!idx){return}
    var s=q.value.toLowerCase().trim();
    if(!s){if(res)res.textContent='';say('输入关键词检索标题 / 标签 / 正文');return}
    var hits=[];
    for(var k=0;k<idx.length&&hits.length<50;k++){
      var e=idx[k];
      if((e.title+' '+(e.tags||[]).join(' ')+' '+e.body).toLowerCase().indexOf(s)>=0)hits.push(e);
    }
    say(hits.length+' 条命中 / MATCHES');
    render(hits);
  });
}
})();"#;

// ---------------------------------------------------------------- 组件

pub fn rail(active: &str, root: &str) -> String {
    let mut items = String::new();
    for (href, idx, label) in NAV {
        let key = href.trim_end_matches(".html");
        let cur = if key == active {
            " aria-current=\"page\""
        } else {
            ""
        };
        items.push_str(&format!(
            "<li><a class=\"rail-link\" href=\"{root}{href}\"{cur}><span class=\"rail-idx\">{idx}</span><span class=\"rail-label\">{label}</span></a></li>"
        ));
    }
    format!("<nav class=\"rail\" aria-label=\"档案导航\"><ul class=\"rail-list\">{items}</ul></nav>")
}

/// 页面级舞台：唯一的大字号标识符 + 仪器 + 行动件
pub fn stage(kicker: &str, ident: &str, meta: &str, aside: &str, actions: &str, reveal: &str) -> String {
    let aside = if aside.is_empty() {
        String::new()
    } else {
        format!("<div class=\"stage-aside\">{aside}</div>")
    };
    let actions = if actions.is_empty() {
        String::new()
    } else {
        format!("<div class=\"stage-actions\">{actions}</div>")
    };
    format!(
        "<section class=\"stage {reveal}\" aria-label=\"档案标识\">\
<div class=\"stage-layer\" aria-hidden=\"true\"></div>\
<div class=\"stage-body\"><p class=\"stage-kicker\">{kicker}</p><h1 class=\"stage-id\">{ident}</h1>\
<p class=\"stage-meta\">{meta}</p>{actions}</div>{aside}</section>"
    )
}

/// 分布圆环的一段：label 是汉字标签，en 是拉丁微标签，两者排版规则不同
pub struct Seg {
    pub label: &'static str,
    pub en: &'static str,
    pub n: usize,
    /// 0 = 中性、1 = 信号（唯一主行动）、2 = 异常态
    pub kind: u8,
    /// 中性段的描边不透明度：与图例色块一一对应
    pub op: &'static str,
}

const RING_R: f64 = 54.0;

pub fn ring(segs: &[Seg]) -> String {
    let total: usize = segs.iter().map(|s| s.n).sum();
    let mut desc = String::new();
    for (i, s) in segs.iter().enumerate() {
        if i > 0 {
            desc.push('、');
        }
        desc.push_str(&format!("{} {}", s.label, s.n));
    }
    let aria = format!("库内分布：{desc}，共 {total} 条");
    let mut arcs = String::new();
    if total > 0 {
        let circ = std::f64::consts::TAU * RING_R;
        let mut acc = 0f64;
        for s in segs {
            if s.n == 0 {
                continue;
            }
            let share = circ * (s.n as f64) / (total as f64);
            let len = if share > 5.0 { share - 4.0 } else { share };
            let stroke = match s.kind {
                1 => "var(--ark-signal)",
                2 => "var(--ark-state)",
                _ => "var(--ark-stage-ink)",
            };
            arcs.push_str(&format!(
                "<circle cx=\"66\" cy=\"66\" r=\"54\" fill=\"none\" stroke=\"{stroke}\" stroke-opacity=\"{op}\" stroke-width=\"6\" stroke-dasharray=\"{len:.2} {rest:.2}\" stroke-dashoffset=\"{off:.2}\" transform=\"rotate(-90 66 66)\"/>",
                op = s.op,
                rest = circ - len,
                off = -acc,
            ));
            acc += share;
        }
    }
    format!(
        "<svg class=\"ring\" viewBox=\"0 0 132 132\" role=\"img\" aria-label=\"{aria}\">\
<circle cx=\"66\" cy=\"66\" r=\"54\" fill=\"none\" stroke=\"var(--ark-stage-line)\" stroke-width=\"6\"/>\
{arcs}<text class=\"ring-num\" x=\"66\" y=\"64\" text-anchor=\"middle\">{total}</text>\
<text class=\"ring-cap\" x=\"66\" y=\"80\" text-anchor=\"middle\">ENTRIES</text></svg>"
    )
}

pub fn legend(segs: &[Seg], overdue: usize) -> String {
    let mut li = String::new();
    for s in segs {
        let cls = match s.kind {
            1 => "swatch is-signal",
            2 => "swatch is-state",
            _ => "swatch",
        };
        li.push_str(&format!(
            "<li><span class=\"{cls}\" style=\"opacity:{op}\"></span><span>{label}</span>\
<span class=\"en\">{en}</span><b>{n}</b></li>",
            op = if s.kind == 0 { s.op } else { "1" },
            label = s.label,
            en = s.en,
            n = s.n,
        ));
    }
    if overdue > 0 {
        // 未完成待办的子集：标注清楚，不参与求和
        li.push_str(&format!(
            "<li><span class=\"swatch is-state\"></span><span>其中逾期</span>\
<span class=\"en\">OVERDUE</span><b>{overdue}</b></li>"
        ));
    }
    format!("<ul class=\"legend\">{li}</ul>")
}

/// 最近 N 天捕获活动：逐日真实条数
pub fn activity(days: &[(String, usize)]) -> String {
    let hits: usize = days.iter().map(|d| d.1).sum();
    let win = days.len();
    let max = days.iter().map(|d| d.1).max().unwrap_or(0).max(1);
    let mut bars = String::new();
    for (d, n) in days {
        let h = 3 + (n * 27 / max).min(27);
        bars.push_str(&format!(
            "<span class=\"activity-day{has}\" style=\"height:{h}px\" title=\"{d} · {n} 条\"></span>",
            has = if *n > 0 { " has" } else { "" },
        ));
    }
    format!(
        "<div class=\"stage-readout\"><span>最近 {win} 天捕获</span><span class=\"en\">CAPTURE</span>\
<div class=\"activity\" role=\"img\" aria-label=\"最近 {win} 天共捕获 {hits} 条\">{bars}</div></div>"
    )
}

/// 页面级微标签：汉字走 UI 栈，拉丁部分走等宽大写字距（两者排版规则不同）
/// mark 是产品词标，带 .mark 供首次访问时的彩蛋替换（与词标同为 9 字符，切换不产生位移）
pub fn kicker(cjk: &str, en: &str) -> String {
    format!(
        "<span class=\"mark\" aria-hidden=\"true\">{mark}</span> <span>{cjk}</span> <span class=\"en\">{en}</span>",
        mark = MARK
    )
}

/// 年表刻度：按 created 时间等比落位；跨度不足 7 天时只留两个端点
pub fn scale(entries: &[&Entry]) -> String {
    let mut times: Vec<i64> = Vec::new();
    for e in entries {
        if let Some(t) = parse_created(&e.fm.created) {
            times.push(t);
        }
    }
    if times.len() < 2 {
        return String::new();
    }
    let min = *times.iter().min().unwrap();
    let max = *times.iter().max().unwrap();
    let span_days = (max - min) as f64 / 86400.0;
    let mut ticks = String::new();
    if span_days >= 7.0 && max > min {
        for t in &times {
            let pct = (*t - min) as f64 / (max - min) as f64 * 100.0;
            ticks.push_str(&format!("<span class=\"scale-tick\" style=\"top:{pct:.2}%\"></span>"));
        }
    }
    let newest = short_date(&entries.first().map(|e| e.fm.created.clone()).unwrap_or_default());
    let oldest = short_date(&entries.last().map(|e| e.fm.created.clone()).unwrap_or_default());
    format!(
        "<div class=\"scale\" aria-hidden=\"true\">{ticks}\
<span class=\"scale-tick is-end is-top\"></span><span class=\"scale-tick is-end is-bottom\"></span>\
<span class=\"scale-end is-top\">{newest}</span><span class=\"scale-end is-bottom\">{oldest}</span></div>"
    )
}

fn short_date(s: &str) -> String {
    let f = fmt_created(s);
    if f.len() >= 10 {
        f[5..10].to_string()
    } else {
        f
    }
}

pub fn band(id: &str, code: &str, title: &str, sub: &str, body: &str, cls: &str) -> String {
    let id = if id.is_empty() {
        String::new()
    } else {
        format!(" id=\"{id}\"")
    };
    let sub = if sub.is_empty() {
        String::new()
    } else {
        format!("<span class=\"band-sub\">{sub}</span>")
    };
    format!(
        "<section class=\"band {cls}\"{id}><div class=\"band-head\"><span class=\"band-code\">{code}</span>\
<h2 class=\"band-title\">{title}</h2>{sub}<span class=\"band-rule\" aria-hidden=\"true\"></span></div>\
<div class=\"band-body\">{body}</div></section>"
    )
}

fn is_done(e: &Entry) -> bool {
    e.fm.type_ == "todo" && e.fm.status.as_deref() == Some("done")
}

fn is_overdue(e: &Entry, today: &str) -> bool {
    e.fm.type_ == "todo"
        && !is_done(e)
        && e.fm.due.as_deref().map(|d| d < today).unwrap_or(false)
}

/// 行的状态列：open / done / 逾期，都是真实状态
fn row_state(e: &Entry, today: &str) -> String {
    if e.fm.type_ != "todo" {
        return String::new();
    }
    if is_done(e) {
        let d = e.fm.done.as_deref().unwrap_or("");
        let d = if d.is_empty() {
            String::new()
        } else {
            format!(" <span class=\"num\">{}</span>", short_date(d))
        };
        return format!(
            "<span class=\"badge is-done\"><span class=\"badge-mark\"></span><span>完成</span>\
<span class=\"en\">DONE</span>{d}</span>"
        );
    }
    let due = match e.fm.due.as_deref() {
        Some(_) if is_overdue(e, today) => format!(
            "<span class=\"badge is-overdue\"><span class=\"badge-mark\"></span><span>逾期</span>\
<span class=\"en\">DUE</span> <span class=\"num\">{}</span></span>",
            short_date(e.fm.due.as_deref().unwrap_or(""))
        ),
        Some(d) => format!(
            "<span class=\"badge\"><span>截止</span><span class=\"en\">DUE</span> <span class=\"num\">{}</span></span>",
            short_date(d)
        ),
        None => String::new(),
    };
    format!(
        "<span class=\"badge is-open\"><span class=\"badge-mark\"></span><span>进行中</span>\
<span class=\"en\">OPEN</span></span>{due}"
    )
}

/// 档案行：日期 + 标题 + 类型 + 标签 + 状态。标签与状态都换行，绝不裁切。
pub fn entry_row(e: &Entry, root: &str, show_dir: bool, today: &str) -> String {
    let tags: String = e
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
        .collect::<Vec<_>>()
        .join("");
    let facets = if tags.is_empty() {
        String::new()
    } else {
        format!("<span class=\"row-facets\">{tags}</span>")
    };
    let kind = if show_dir {
        format!("<span class=\"row-kind\">{}</span>", esc(&e.dir))
    } else {
        String::new()
    };
    let done = if is_done(e) { " is-done" } else { "" };
    let title_cls = if is_done(e) {
        "row-title is-done"
    } else {
        "row-title"
    };
    format!(
        "<li class=\"row{done}\"><span class=\"row-date\">{date}</span>\
<div class=\"row-main\"><a class=\"{title_cls}\" href=\"{root}pages/{stem}.html\">{title}</a>{kind}{facets}</div>\
<div class=\"row-side\">{state}</div></li>",
        date = esc(&fmt_created(&e.fm.created)),
        stem = esc(&e.stem),
        title = esc(&e.fm.title),
        state = row_state(e, today),
    )
}

pub fn action(href: &str, label: &str, cls: &str) -> String {
    format!("<a class=\"act {cls}\" href=\"{href}\">{label}</a>")
}

// ---------------------------------------------------------------- 页面

pub struct Page {
    pub title: String,
    pub nav: String,
    pub stage: String,
    pub body: String,
}

impl Page {
    pub fn new(title: &str, nav: &str) -> Self {
        Page {
            title: title.to_string(),
            nav: nav.to_string(),
            stage: String::new(),
            body: String::new(),
        }
    }
    pub fn stage(mut self, s: String) -> Self {
        self.stage = s;
        self
    }
    pub fn body(mut self, b: String) -> Self {
        self.body = b;
        self
    }
}

pub fn render(p: &Page, built: &str, count_line: &str) -> String {
    let root = root_of(&p.nav);
    format!(
        "<!DOCTYPE html>\n<html lang=\"zh\" data-ark-theme=\"exa\" data-ark-depth=\"moderate\" data-ark-appearance=\"auto\" data-ark-layout=\"list\">\n\
<head>\n<meta charset=\"utf-8\">\n<script>{BOOT}</script>\n\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n\
<title>{title} · MindCache</title>\n<meta name=\"color-scheme\" content=\"light dark\">\n\
<meta name=\"theme-color\" content=\"#f3f2ef\" media=\"(prefers-color-scheme: light)\">\n\
<meta name=\"theme-color\" content=\"#080914\" media=\"(prefers-color-scheme: dark)\">\n\
<link rel=\"icon\" href=\"data:,\">\n<link rel=\"stylesheet\" href=\"{root}style.css\">\n</head>\n<body>\n\
<a class=\"skip\" href=\"#main\">跳到主要内容 / SKIP TO CONTENT</a>\n\
<div class=\"shell\">\n\
<header class=\"topbar\"><p class=\"brand\"><b>MindCache</b><span>个人档案库</span><span class=\"en\">/ PERSONAL ARCHIVE</span></p>\
<div class=\"topbar-tools\"><span class=\"hint\">{count_line}</span>\
<button class=\"act\" id=\"layoutbtn\" type=\"button\">视图 VIEW · LIST</button>\
<button class=\"act\" id=\"appearancebtn\" type=\"button\">外观 THEME · AUTO</button></div></header>\n\
{nav}\n<main id=\"main\" class=\"field\">\n{stage}\n{body}\n</main>\n\
<footer class=\"statusbar\"><span>MindCache <b>v{version}</b></span>\
<span><span class=\"en\">LAST BUILD</span> <span>上次构建</span> <b>{built}</b></span>\
<span><span>契约</span> <span class=\"en\">CONTRACT</span> <b>EXA / MODERATE</b></span></footer>\n</div>\n\
<script>{js}</script>\n</body>\n</html>\n",
        BOOT = BOOT,
        js = JS,
        title = esc(&p.title),
        nav = rail(&p.nav, root),
        stage = p.stage,
        body = p.body,
        version = crate::VERSION,
    )
}

