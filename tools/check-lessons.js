// 比對 js/data.js 與 lessons/*.md：標題、總時間、每一步的時間與口令稿必須逐字相同。
// 用法：node tools/check-lessons.js（不需要安裝任何套件）
'use strict';
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const root = path.join(__dirname, '..');
const sandbox = { window: {} };
vm.runInNewContext(fs.readFileSync(path.join(root, 'js/data.js'), 'utf8'), sandbox, { filename: 'js/data.js' });
const D = sandbox.window.NIGHT_YOGA;

let failures = 0;
const fail = (msg) => { failures += 1; console.log('  FAIL ' + msg); };

for (const L of D.lessons) {
  console.log('課程 ' + L.id + ' ' + L.title + '  ←→  ' + L.file);
  const md = fs.readFileSync(path.join(root, L.file), 'utf8');
  const fm = md.match(/^---\n([\s\S]*?)\n---/)[1];
  const mdId = fm.match(/id: (\S+)/)[1];
  const mdTitle = fm.match(/title: (.+)/)[1].trim();
  const mdMinutes = Number(fm.match(/minutes: (\d+)/)[1]);

  if (mdId !== L.id) fail('id：網頁 ' + L.id + '，Markdown ' + mdId);
  if (mdTitle !== L.title) fail('標題：網頁「' + L.title + '」，Markdown「' + mdTitle + '」');
  const total = L.steps.reduce((a, s) => a + s.min, 0);
  if (total !== mdMinutes) fail('總時間：網頁各步相加 ' + total + '，Markdown ' + mdMinutes);

  // 口令稿：每個 ### 小節底下的「> 」行；標題含「選配」的小節不算正式步驟
  const sections = [];
  let cur = null;
  for (const line of md.split('\n')) {
    if (/^#{1,3} /.test(line)) {
      cur = line.startsWith('### ') ? { head: line, cues: [] } : null;
      if (cur) sections.push(cur);
    } else if (cur && line.startsWith('> ')) {
      cur.cues.push(line.slice(2).trim());
    }
  }
  const core = sections.filter((s) => s.cues.length && !s.head.includes('選配'));
  if (core.length !== L.steps.length) {
    fail('步驟數：網頁 ' + L.steps.length + '，Markdown 口令稿 ' + core.length);
    continue;
  }
  core.forEach((sec, i) => {
    const step = L.steps[i];
    const m = sec.head.match(/(\d+(?:\.\d+)?) 分鐘/);
    if (!m || Number(m[1]) !== step.min) fail('第 ' + (i + 1) + ' 步時間：網頁 ' + step.min + ' 分鐘，Markdown「' + sec.head + '」');
    if (sec.cues.length !== step.cues.length) {
      fail('第 ' + (i + 1) + ' 步口令行數：網頁 ' + step.cues.length + '，Markdown ' + sec.cues.length);
      return;
    }
    sec.cues.forEach((c, j) => {
      if (c !== step.cues[j]) fail('第 ' + (i + 1) + ' 步第 ' + (j + 1) + ' 行不同\n    網頁：' + step.cues[j] + '\n    Markdown：' + c);
    });
  });
}

// 「停下來的時機」要和 Markdown 一致
for (const L of D.lessons) {
  const md = fs.readFileSync(path.join(root, L.file), 'utf8');
  const stop = (md.match(/## 停下來的時機\n\n(.+)/) || [])[1];
  if (stop !== D.stop) fail('「停下來的時機」：網頁「' + D.stop + '」，' + L.file + '「' + stop + '」');
}

console.log(failures ? '\n發現 ' + failures + ' 處不一致' : '\n全部一致');
process.exit(failures ? 1 : 0);
