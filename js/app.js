// 課程播放：說明頁 → 逐步倒數與呼吸引導 → 完成。內容來自 js/data.js。
(function () {
  'use strict';

  var D = window.NIGHT_YOGA;
  var LESSON = D.lessons[0];
  var STEPS = LESSON.steps;
  var INHALE = D.breath.inhaleMs, CYCLE = D.breath.cycleMs, MIN_SCALE = D.breath.minScale;
  var TOTAL = STEPS.reduce(function (a, s) { return a + s.min * 60000; }, 0);

  var $ = function (id) { return document.getElementById(id); };
  var app = $('app');
  var views = { intro: $('intro'), play: $('play'), done: $('done') };
  var idx = 0, elapsed = 0, running = false, last = 0, timerId = 0, rafId = 0, wake = null;
  var reduce = !!(window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches);

  function now() { return (window.performance && performance.now) ? performance.now() : Date.now(); }
  function fmt(ms) { var s = Math.max(0, Math.ceil(ms / 1000)); return Math.floor(s / 60) + ':' + ('0' + (s % 60)).slice(-2); }
  function dur(i) { return STEPS[i].min * 60000; }
  function ease(t) { return t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2; }
  function sec(ms) { return String(ms / 1000); }
  function show(name) {
    Object.keys(views).forEach(function (k) { views[k].hidden = (k !== name); });
    app.setAttribute('data-view', name);
    try { window.scrollTo(0, 0); } catch (e) {}
  }

  // 說明頁：標題、準備、流程、停下來的時機、小字
  document.title = '夜間瑜珈 ' + LESSON.title;
  $('lessonNo').textContent = '夜間瑜珈　課程 ' + LESSON.id;
  $('lessonTitle').textContent = LESSON.title;
  $('lessonMeta').textContent = Math.round(TOTAL / 60000) + ' 分鐘　' + STEPS.length + ' 個動作　' + LESSON.tagline;
  LESSON.prep.forEach(function (t) {
    var p = document.createElement('p'); p.textContent = t; $('prep').appendChild(p);
  });
  $('stop').textContent = D.stop;
  var notes = $('notes');
  D.notes.forEach(function (t) {
    var p = document.createElement('p');
    p.textContent = t.replace('{inhale}', sec(INHALE)).replace('{exhale}', sec(CYCLE - INHALE));
    notes.appendChild(p);
  });

  var list = $('steplist');
  STEPS.forEach(function (s) {
    var li = document.createElement('li');
    var nm = document.createElement('span'); nm.className = 'nm'; nm.textContent = s.name;
    var pose = document.createElement('span'); pose.className = 'pose'; pose.textContent = s.pose;
    var left = document.createElement('span'); left.appendChild(nm); left.appendChild(pose);
    var mins = document.createElement('span'); mins.className = 'mins'; mins.textContent = s.min + ' 分鐘';
    var src = document.createElement('span'); src.className = 'src'; src.appendChild(document.createTextNode('來源：'));
    s.src.forEach(function (p, i) {
      if (i) src.appendChild(document.createTextNode('、'));
      var a = document.createElement('a'); a.href = p[1]; a.target = '_blank'; a.rel = 'noopener'; a.textContent = p[0];
      src.appendChild(a);
    });
    li.appendChild(left); li.appendChild(mins); li.appendChild(src);
    list.appendChild(li);
  });

  // 進度條
  var bar = $('bar');
  STEPS.forEach(function (s) {
    var seg = document.createElement('div'); seg.className = 'seg'; seg.style.flex = String(s.min);
    seg.appendChild(document.createElement('i')); bar.appendChild(seg);
  });
  var fills = bar.querySelectorAll('i');

  // 呼吸引導圈偏好
  var breathOn = true;
  try { if (localStorage.getItem('ny-breath') === '0') breathOn = false; } catch (e) {}
  $('breathToggle').checked = breathOn;
  function applyBreath() { app.classList.toggle('nobreath', !breathOn); }
  applyBreath();
  $('breathToggle').addEventListener('change', function () {
    breathOn = this.checked; applyBreath();
    try { localStorage.setItem('ny-breath', breathOn ? '1' : '0'); } catch (e) {}
  });

  function renderStep() {
    var s = STEPS[idx];
    $('of').textContent = '第 ' + (idx + 1) + ' 步，共 ' + STEPS.length + ' 步';
    $('stepName').textContent = s.name;
    $('stepPose').textContent = s.pose;
    var c = $('cues'); c.textContent = '';
    s.cues.forEach(function (t) { var p = document.createElement('p'); p.textContent = t; c.appendChild(p); });
    $('next').textContent = (idx === STEPS.length - 1) ? '完成' : '下一步';
  }

  function draw() {
    var d = dur(idx), p = elapsed % CYCLE, inhale = p < INHALE, scale;
    if (reduce) scale = 0.86;
    else scale = MIN_SCALE + (1 - MIN_SCALE) * (inhale ? ease(p / INHALE) : 1 - ease((p - INHALE) / (CYCLE - INHALE)));
    $('orb').style.transform = 'scale(' + scale.toFixed(4) + ')';
    $('lab').textContent = running ? (inhale ? '吸氣' : '吐氣') : '暫停中';
    $('tStep').textContent = fmt(d - elapsed);
    var before = 0; for (var i = 0; i < idx; i++) before += dur(i);
    $('tAll').textContent = '全程還剩 ' + fmt(TOTAL - before - elapsed);
    for (var j = 0; j < fills.length; j++) {
      fills[j].style.width = (j < idx ? 100 : j === idx ? Math.min(100, elapsed / d * 100) : 0).toFixed(2) + '%';
    }
  }

  function tick() {
    var t = now();
    if (running) elapsed += t - last;
    last = t;
    // 計時器被系統拉長（例如手機切到背景）時，把多出來的時間帶到下一步，不丟掉
    while (running && elapsed >= dur(idx)) {
      if (idx >= STEPS.length - 1) { finish(); return; }
      elapsed -= dur(idx); idx += 1; renderStep();
    }
    draw();
  }
  function frame() { tick(); if (running) rafId = requestAnimationFrame(frame); }

  function advance() {
    if (idx >= STEPS.length - 1) { finish(); return; }
    idx += 1; elapsed = 0; last = now(); renderStep(); draw();
  }
  function lockOn() {
    try { if ('wakeLock' in navigator) navigator.wakeLock.request('screen').then(function (l) { wake = l; }, function () {}); } catch (e) {}
  }
  function lockOff() { try { if (wake) wake.release(); } catch (e) {} wake = null; }
  function setRunning(v) {
    running = v; last = now();
    $('pause').textContent = v ? '暫停' : '繼續';
    cancelAnimationFrame(rafId);
    if (v) { rafId = requestAnimationFrame(frame); lockOn(); } else { draw(); }
  }
  function begin() {
    idx = 0; elapsed = 0; renderStep(); show('play'); draw(); setRunning(true);
    if (!timerId) timerId = setInterval(tick, 250);
  }
  function finish() {
    running = false; cancelAnimationFrame(rafId); lockOff(); show('done');
  }
  function quit() {
    running = false; cancelAnimationFrame(rafId); lockOff(); show('intro');
  }

  $('start').addEventListener('click', begin);
  $('again').addEventListener('click', begin);
  $('pause').addEventListener('click', function () { setRunning(!running); });
  $('next').addEventListener('click', advance);
  $('quit').addEventListener('click', quit);
  document.addEventListener('visibilitychange', function () {
    if (document.visibilityState === 'visible' && running) { tick(); lockOn(); }
  });
})();
