// dashboard 交互回归测试：node + jsdom 跑页面内联 JS。
// jsdom 不在仓库依赖里（AGENTS.md 约定装到临时目录），用法：
//   npm install jsdom --prefix /tmp/mindjsdom
//   NODE_PATH=/tmp/mindjsdom/node_modules node tests/dashboard.mjs <dist-dir>
// 断言覆盖：折叠、检索（命中数/链接/零结果）、tag chip ↔ 锚点、外观与布局持久化、
// 旧键迁移、无 fetch 时的降级、每行链接都指向真实文件。

import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
let JSDOM;
try {
  ({ JSDOM } = require('jsdom'));
} catch (e) {
  console.error('需要 jsdom：npm install jsdom --prefix <临时目录> 后设 NODE_PATH 指向其 node_modules');
  process.exit(2);
}

const DIST = process.argv[2] || process.env.MIND_DIST;
if (!DIST || !fs.existsSync(DIST)) {
  console.error('用法: node tests/dashboard.mjs <dist-dir>（缺参数或目录不存在）');
  process.exit(2);
}

let failed = 0;
let passed = 0;
function ok(cond, label, extra) {
  if (cond) {
    passed++;
    console.log('  ok   ' + label);
  } else {
    failed++;
    console.log('  FAIL ' + label + (extra ? '  → ' + extra : ''));
  }
}

async function load(file, { seed, fetchSearch = false, wait = 60 } = {}) {
  const html = fs.readFileSync(path.join(DIST, file), 'utf8');
  const dom = new JSDOM(html, {
    url: 'http://localhost/' + file,
    runScripts: 'dangerously',
    pretendToBeVisual: true,
    // seed 必须早于页面脚本写入 localStorage；fetch 依赖同理
    beforeParse(window) {
      if (seed) for (const [k, v] of Object.entries(seed)) window.localStorage.setItem(k, v);
      if (fetchSearch) {
        window.fetch = (url) =>
          Promise.resolve({
            json: () => Promise.resolve(JSON.parse(fs.readFileSync(path.join(DIST, url), 'utf8'))),
          });
      }
    },
  });
  await new Promise((r) => setTimeout(r, wait));
  return dom.window;
}

const readJson = (f) => JSON.parse(fs.readFileSync(path.join(DIST, f), 'utf8'));

console.log('# dashboard 交互测试 · ' + DIST);

// ---- 1. 折叠：默认 8 条、展开后按钮消失
{
  const w = await load('index.html');
  const doc = w.document;
  const list = doc.querySelector('.rows[data-fold="8"]');
  ok(!!list, 'HomePage 存在 data-fold=8 列表');
  const rows = list ? list.querySelectorAll(':scope > .row') : [];
  const hidden = list ? list.querySelectorAll('.hidden-row') : [];
  ok(rows.length > 8, '折叠列表条目数 > 8（' + rows.length + '）');
  ok(hidden.length === rows.length - 8, '默认折叠数 = 总数 - 8（' + hidden.length + '）');
  const btn = doc.querySelector('.foldbtn');
  ok(!!btn, '存在展开按钮 .foldbtn');
  ok(!!btn && btn.textContent.includes('+' + (rows.length - 8)), '按钮携带真实增量 (+' + (rows.length - 8) + ')');
  if (btn) btn.dispatchEvent(new w.Event('click', { bubbles: true }));
  ok(doc.querySelectorAll('.hidden-row').length === 0, '点击后全部展开');
  ok(!doc.querySelector('.foldbtn'), '展开后按钮移除（不死行）');
}

// ---- 2. 检索：命中数、链接、零结果、无 fetch 降级
{
  const idx = readJson('search.json').entries;
  const w = await load('index.html', { fetchSearch: true });
  const doc = w.document;
  const q = doc.getElementById('q');
  ok(!!q, '存在检索输入框 #q');
  ok(!!doc.querySelector('label[for="q"]'), '#q 有 <label for>（不靠 placeholder 当标签）');
  // 关键词取自真实索引（标题的末两字），不写死，避免依赖特定 fixture 内容
  const probe = (idx[0].title || '').trim().slice(-2) || 'a';
  q.value = probe;
  q.dispatchEvent(new w.Event('input', { bubbles: true }));
  const hits = doc.querySelectorAll('#results .row');
  const expect = idx.filter((e) => (e.title + ' ' + (e.tags || []).join(' ') + ' ' + e.body).toLowerCase().includes(probe.toLowerCase())).length;
  ok(expect > 0 && hits.length === expect, '命中数与索引一致（' + hits.length + ' / ' + expect + '）');
  const first = doc.querySelector('#results .row-title');
  const target = idx.find((e) => e.stem === (first ? first.getAttribute('href').replace('pages/', '').replace('.html', '') : ''));
  ok(!!target, '命中行链接指向真实条目');
  ok(!!first && !!target && first.textContent === target.title, '链接文本用 textContent 写入且与索引一致');
  ok((doc.getElementById('qstatus').textContent || '').includes(String(expect)), '状态行 role=status 报出命中数');
  q.value = 'zzzzzz';
  q.dispatchEvent(new w.Event('input', { bubbles: true }));
  ok(doc.querySelectorAll('#results .row').length === 0, '零结果不产生行');
  ok((doc.getElementById('qstatus').textContent || '').includes('0 条命中'), '零结果有明确文案');

  const noFetch = await load('index.html');
  ok(!noFetch.document.getElementById('q'), '无 fetch 时检索面板整体移除（不留坏控件）');
}

// ---- 3. tag chip ↔ 锚点
{
  const w = await load('tags.html');
  const doc = w.document;
  const chips = [...doc.querySelectorAll('.chip')];
  const ids = new Set([...doc.querySelectorAll('.band[id]')].map((b) => b.id));
  ok(chips.length > 0, '标签索引有 chip（' + chips.length + '）');
  const broken = chips.filter((c) => !ids.has(decodeURIComponent(c.getAttribute('href').slice(1))));
  ok(broken.length === 0, '每个 chip 都有对应锚点', broken.map((c) => c.getAttribute('href')).join(','));
  // 条目里的 tag 链接指向 tags.html#锚点，且该锚点存在
  const rowTags = [...doc.querySelectorAll('.row-facets .tag')];
  ok(rowTags.every((t) => t.getAttribute('href').startsWith('tags.html#')), '条目内 tag 链接指向聚合页锚点');
}

// ---- 4. 外观 / 布局持久化与旧键迁移
{
  const w = await load('index.html');
  const doc = w.document;
  const root = doc.documentElement;
  const ab = doc.getElementById('appearancebtn');
  const lb = doc.getElementById('layoutbtn');
  ok(root.getAttribute('data-ark-theme') === 'exa', '根属性 data-ark-theme=exa（family 固定）');
  ok(root.getAttribute('data-ark-depth') === 'moderate', '根属性 data-ark-depth=moderate');
  ok(!!ab && !!lb, '存在外观与布局两个控件');
  ab.dispatchEvent(new w.Event('click', { bubbles: true }));
  ok(root.getAttribute('data-ark-appearance') === 'light', '点击后外观 = light');
  ok(w.localStorage.getItem('mind-appearance') === 'light', '外观写入 mind-appearance');
  ab.dispatchEvent(new w.Event('click', { bubbles: true }));
  ab.dispatchEvent(new w.Event('click', { bubbles: true }));
  ok(root.getAttribute('data-ark-appearance') === null, '三轮后回到 auto（属性移除）');
  ok(ab.textContent.includes('AUTO'), '按钮文本报出当前值');
  lb.dispatchEvent(new w.Event('click', { bubbles: true }));
  ok(root.getAttribute('data-ark-layout') === 'grid', '布局切到 grid');
  ok(w.localStorage.getItem('mind-layout') === 'grid', '布局写入 mind-layout');

  const legacy = await load('index.html', { seed: { 'mind-theme': 'endfield', 'mind-density': '4' } });
  ok(legacy.document.documentElement.getAttribute('data-ark-appearance') === 'dark', '旧 mind-theme=endfield 迁移为 dark');
  ok(legacy.document.documentElement.getAttribute('data-ark-layout') === 'grid', '旧 mind-density=4 迁移为 grid');
  ok(legacy.localStorage.getItem('mind-theme') === null, '旧键清理');
}

// ---- 5. 结构：单 h1、地标、无占位符文本、行链接都落地
{
  // 只取真实文本节点（跳过 script/style，否则页面脚本源码会被误判成占位符）
  const visibleText = (doc) => {
    let out = '';
    for (const el of doc.body.querySelectorAll('*')) {
      if (/^(script|style|template|noscript)$/i.test(el.tagName)) continue;
      for (const n of el.childNodes) if (n.nodeType === 3) out += n.nodeValue + ' ';
    }
    return out;
  };
  const pages = ['index.html', 'inbox.html', 'todo.html', 'ideas.html', 'notes.html', 'tags.html', ...fs.readdirSync(path.join(DIST, 'pages')).map((f) => 'pages/' + f)];
  let h1bad = [];
  let landmarkBad = [];
  let undefinedText = [];
  let dangling = [];
  let brandBad = [];
  for (const f of pages) {
    const w = await load(f, { wait: 5 });
    const doc = w.document;
    const h1 = doc.querySelectorAll('h1');
    if (h1.length !== 1) h1bad.push(f + ':' + h1.length);
    if (!doc.querySelector('header') || !doc.querySelector('main') || !doc.querySelector('footer') || !doc.querySelector('nav')) landmarkBad.push(f);
    if (/undefined|NaN|\{\}/.test(visibleText(doc))) undefinedText.push(f);
    // 只检查外壳装饰文本：正文、标题、舞台大字号标识符都是用户数据
    // （详情页的 .stage-id 就是条目标题），出现什么词都不算我们的问题
    const chrome = doc.querySelector('header').textContent + doc.querySelector('.rail').textContent
      + doc.querySelector('footer').textContent
      + (doc.querySelector('.stage-kicker')?.textContent || '')
      + (doc.querySelector('.stage-meta')?.textContent || '')
      + (doc.querySelector('.stage-actions')?.textContent || '');
    if (/END\s?FIELD/i.test(chrome)) brandBad.push(f);
    for (const a of doc.querySelectorAll('a[href]')) {
      const href = a.getAttribute('href');
      if (/^(https?:|#|mailto:)/.test(href)) continue;
      const file = href.split('#')[0];
      if (!file) continue;
      const p = path.resolve(path.dirname(path.join(DIST, f)), file);
      if (!fs.existsSync(p)) dangling.push(f + ' → ' + href);
    }
  }
  ok(h1bad.length === 0, '每页恰好一个 h1', h1bad.join(','));
  ok(landmarkBad.length === 0, '每页具备 header/main/footer/nav 地标', landmarkBad.join(','));
  ok(undefinedText.length === 0, '无 undefined/NaN/{} 占位文本', undefinedText.join(','));
  ok(dangling.length === 0, '所有站内链接都可落地', dangling.slice(0, 4).join(','));
  ok(brandBad.length === 0, '外壳不再出现借用他产品的字样', brandBad.slice(0, 3).join(','));
}

console.log(`\n# 结果: ${passed} 通过, ${failed} 失败`);
process.exit(failed ? 1 : 0);
