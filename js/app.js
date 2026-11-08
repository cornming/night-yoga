// 課程播放：說明頁 → 逐步倒數、呼吸引導與語音朗讀 → 完成。內容來自 js/data.js。
(function () {
  'use strict';

  var D = window.NIGHT_YOGA;
  var LESSON = D.lessons[0];
  var STEPS = LESSON.steps;
  var INHALE = D.breath.inhaleMs, CYCLE = D.breath.cycleMs, MIN_SCALE = D.breath.minScale;
  var SP = D.speech;
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

  // 動作圖示由 js/poses.js 產生；缺圖或載入失敗時就不顯示，課程照常進行
  function setIcon(box, id) {
    box.textContent = '';
    var svg = (window.makePoseIcon && id) ? window.makePoseIcon(id) : null;
    if (svg) box.appendChild(svg);
  }

  // ───────────── 說明頁 ─────────────
  document.title = '夜間瑜珈 ' + LESSON.title;
  $('lessonNo').textContent = '夜間瑜珈　課程 ' + LESSON.id;
  $('lessonTitle').textContent = LESSON.title;
  $('lessonMeta').textContent = Math.round(TOTAL / 60000) + ' 分鐘　' + STEPS.length + ' 個動作　' + LESSON.tagline;
  LESSON.prep.forEach(function (t) {
    var p = document.createElement('p'); p.textContent = t; $('prep').appendChild(p);
  });
  $('stop').textContent = D.stop;
  $('doneTitle').textContent = D.done[0];
  $('doneSub').textContent = D.done[1];
  var notes = $('notes');
  D.notes.forEach(function (t) {
    var p = document.createElement('p');
    p.textContent = t.replace('{inhale}', sec(INHALE)).replace('{exhale}', sec(CYCLE - INHALE));
    notes.appendChild(p);
  });

  var list = $('steplist');
  STEPS.forEach(function (s) {
    var li = document.createElement('li');
    var tt = document.createElement('span'); tt.className = 'tt';
    var nm = document.createElement('span'); nm.className = 'nm'; nm.textContent = s.name;
    var pose = document.createElement('span'); pose.className = 'pose'; pose.textContent = s.pose;
    tt.appendChild(nm); tt.appendChild(pose);
    var src = document.createElement('span'); src.className = 'src';
    var mins = document.createElement('span'); mins.className = 'mins'; mins.textContent = s.min + ' 分鐘';
    src.appendChild(mins); src.appendChild(document.createTextNode('來源：'));
    s.src.forEach(function (p, i) {
      if (i) src.appendChild(document.createTextNode('、'));
      var a = document.createElement('a'); a.href = p[1]; a.target = '_blank'; a.rel = 'noopener'; a.textContent = p[0];
      src.appendChild(a);
    });
    var ico = document.createElement('div'); ico.className = 'ico'; setIcon(ico, s.icon);
    li.appendChild(tt); li.appendChild(src); li.appendChild(ico);
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

  // ───────────── 語音朗讀（瀏覽器內建的 Web Speech API）─────────────
  // 每一步開頭：報步驟名稱，再依序念口令，句與句之間停 gapMs。念完就安靜，讓使用者自己呼吸。
  // 暫停會取消目前這句，繼續時從被打斷的那句重念。
  var NUM = ['零', '一', '二', '三', '四', '五', '六', '七', '八', '九', '十'];
  var synth = ('speechSynthesis' in window && 'SpeechSynthesisUtterance' in window) ? window.speechSynthesis : null;
  // state：init 尚未取得語音清單｜unsupported 瀏覽器不支援｜none 有清單但沒有中文語音｜default 取不到清單，交給瀏覽器｜voice 已選定
  var sp = { state: synth ? 'init' : 'unsupported', voice: null, rank: 0, gen: 0, list: [], pos: 0, needRun: true, timer: 0, utter: null };
  var speechOn = true;
  try { if (localStorage.getItem('ny-speech') === '0') speechOn = false; } catch (e) {}

  function normLang(l) { return String(l || '').toLowerCase().replace(/_/g, '-'); }
  // 1：台灣國語；2：繁體中文；3：其他普通話（例如 zh-CN）；99：不用（粵語、非中文）
  function voiceRank(v) {
    var l = normLang(v.lang);
    if (/^(yue|zh-hk|zh-hant-hk)/.test(l)) return 99;
    if (/^(zh|cmn)-(hant-)?tw$/.test(l)) return 1;
    if (/^(zh|cmn)-hant/.test(l)) return 2;
    if (/^(zh|cmn)(-|$)/.test(l)) return 3;
    return 99;
  }
  var voiceWaited = false;
  function refreshVoice() {
    if (!synth) return;
    var vs = []; try { vs = synth.getVoices() || []; } catch (e) {}
    if (!vs.length) {
      sp.voice = null; sp.rank = 0;
      sp.state = voiceWaited ? 'default' : 'init';
    } else {
      var best = null, bestRank = 99;
      vs.forEach(function (v) { var r = voiceRank(v); if (r < bestRank) { best = v; bestRank = r; } });
      sp.voice = best; sp.rank = bestRank; sp.state = best ? 'voice' : 'none';
    }
    syncSpeechUI();
  }
  function speechUsable() { return speechOn && (sp.state === 'voice' || sp.state === 'default' || sp.state === 'init'); }

  function syncSpeechUI() {
    var info = '';
    if (sp.state === 'unsupported') info = '這個瀏覽器不支援語音朗讀。';
    else if (sp.state === 'none') info = '這台裝置沒有中文語音，無法朗讀。可以到系統設定安裝中文語音。';
    else if (sp.state === 'default') info = '找不到語音清單，交給瀏覽器的預設語音朗讀，不確定是不是中文。';
    else if (sp.state === 'voice') info = '朗讀語音：' + sp.voice.name + '（' + sp.voice.lang + '）' + (sp.rank >= 3 ? '，不是台灣國語語音' : '');
    $('voiceInfo').textContent = info;
    var blocked = (sp.state === 'unsupported' || sp.state === 'none');
    $('speechRow').hidden = (sp.state === 'unsupported');
    $('speechToggle').disabled = blocked;
    $('speechToggle').checked = speechOn && !blocked;
    $('speechBtn').hidden = blocked;
    $('speechBtn').textContent = '朗讀：' + ((speechOn && !blocked) ? '開' : '關');
  }

  function stepList(i) { return ['第' + NUM[i + 1] + '步，' + STEPS[i].name + '。'].concat(STEPS[i].cues); }

  function speakNext() {
    if (!synth || !speechUsable()) return;
    if (sp.needRun && !running) return;
    if (sp.pos >= sp.list.length) return;
    var g = sp.gen;
    var u = new SpeechSynthesisUtterance(sp.list[sp.pos]);
    u.lang = sp.voice ? sp.voice.lang : 'zh-TW';
    if (sp.voice) u.voice = sp.voice;
    u.rate = SP.rate; u.pitch = SP.pitch;
    var finished = false;
    function next() {
      if (finished || g !== sp.gen) return;
      finished = true; sp.pos += 1;
      sp.timer = setTimeout(function () { if (g === sp.gen) speakNext(); }, SP.gapMs);
    }
    u.onend = next;
    u.onerror = function (e) {
      if (g !== sp.gen) return;                                   // 已被取消的舊句子
      if (e && (e.error === 'canceled' || e.error === 'interrupted')) return;
      speechFailed(e);
    };
    sp.utter = u;                                                 // 留住參照，避免部分瀏覽器提早回收而收不到 onend
    try { synth.speak(u); } catch (e) { speechFailed(e); }
  }
  // 取消目前朗讀；回傳取消前是否正在念（有的瀏覽器取消後要稍等才能再念）
  function speechStop() {
    sp.gen += 1; clearTimeout(sp.timer);
    var was = false;
    try { if (synth) { was = !!(synth.speaking || synth.pending); synth.cancel(); } } catch (e) {}
    return was;
  }
  function speechPlay(lines, needRun) {
    var was = speechStop();
    sp.list = lines; sp.pos = 0; sp.needRun = needRun;
    if (!speechUsable()) return;
    var g = sp.gen;
    // 第一次由「開始」按鈕直接呼叫，不能延遲，否則 iOS 會當成沒有使用者操作
    if (was) sp.timer = setTimeout(function () { if (g === sp.gen) speakNext(); }, 80);
    else speakNext();
  }
  function speechFailed(e) {
    speechStop(); speechOn = false; syncSpeechUI();
    var m = $('voiceMsg');
    m.textContent = '朗讀沒有成功（' + ((e && e.error) || '未知原因') + '），已先關閉。計時照常進行。';
    m.hidden = false;
  }
  function setSpeech(v) {
    speechOn = v;
    try { localStorage.setItem('ny-speech', v ? '1' : '0'); } catch (e) {}
    $('voiceMsg').hidden = true;
    syncSpeechUI();
    if (!v) { speechStop(); return; }
    if (app.getAttribute('data-view') !== 'play') return;
    if (running) speechPlay(stepList(idx), true);                 // 播放中打開：從這一步開頭重念
    else { sp.list = stepList(idx); sp.pos = 0; sp.needRun = true; }   // 暫停中打開：繼續時再念
  }

  if (synth) {
    refreshVoice();
    try { synth.addEventListener('voiceschanged', refreshVoice); } catch (e) { synth.onvoiceschanged = refreshVoice; }
    setTimeout(function () { voiceWaited = true; if (sp.state === 'init') refreshVoice(); }, 1500);
  }
  syncSpeechUI();
  $('speechToggle').addEventListener('change', function () { setSpeech(this.checked); });
  $('speechBtn').addEventListener('click', function () { setSpeech(!speechOn); });

  // ───────────── 播放 ─────────────
  function renderStep() {
    var s = STEPS[idx];
    $('of').textContent = '第 ' + (idx + 1) + ' 步，共 ' + STEPS.length + ' 步';
    $('stepName').textContent = s.name;
    $('stepPose').textContent = s.pose;
    setIcon($('poseIcon'), s.icon);
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
    var from = idx;
    while (running && elapsed >= dur(idx)) {
      if (idx >= STEPS.length - 1) { finish(); return; }
      elapsed -= dur(idx); idx += 1; renderStep();
    }
    if (idx !== from) speechPlay(stepList(idx), true);            // 跨了幾步都只念最後停下的那一步
    draw();
  }
  function frame() { tick(); if (running) rafId = requestAnimationFrame(frame); }

  function advance() {
    if (idx >= STEPS.length - 1) { finish(); return; }
    idx += 1; elapsed = 0; last = now(); renderStep(); draw();
    speechPlay(stepList(idx), true);
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
    idx = 0; elapsed = 0; $('voiceMsg').hidden = true;
    renderStep(); show('play'); draw(); setRunning(true);
    speechPlay(stepList(0), true);
    if (!timerId) timerId = setInterval(tick, 250);
  }
  function finish() {
    running = false; cancelAnimationFrame(rafId); lockOff(); show('done');
    speechPlay(D.done.slice(), false);
  }
  function quit() {
    running = false; cancelAnimationFrame(rafId); lockOff(); speechStop(); show('intro');
  }

  $('start').addEventListener('click', begin);
  $('again').addEventListener('click', begin);
  $('pause').addEventListener('click', function () {
    var v = !running;
    setRunning(v);
    if (v) speakNext(); else speechStop();                        // 暫停就停止朗讀；繼續時從被打斷的那句重念
  });
  $('next').addEventListener('click', advance);
  $('quit').addEventListener('click', quit);
  document.addEventListener('visibilitychange', function () {
    if (document.visibilityState === 'visible' && running) { tick(); lockOn(); }
  });
})();
