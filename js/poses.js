// 動作圖示：從上方看躺在床上的人，頭在左、腳在右。用關節角度算出來的簡單剪影，不是照片或影像。
// 編號對應 poses.md（P01、P02…）。新增動作時，在 POSES 加一筆即可。
// 角度以「身體下半側」為準（0° 朝腳的方向，正值朝畫面下方）；上半側會自動左右鏡射。
(function () {
  'use strict';

  var NS = 'http://www.w3.org/2000/svg';

  // 畫布 184×120，身體中線在 y = 60
  var G = {
    head: { x: 38, r: 8.5 },
    torso: { x1: 60, x2: 92, w: 24 },
    shoulder: { x: 56, dy: 13 },
    hip: { x: 96, dy: 7 },
    arm: [22, 20],        // 上臂、前臂長度
    leg: [38, 36],        // 大腿、小腿長度
    limb: 8, hand: 4.5, foot: 5
  };

  var POSES = {
    // 來源：poses.md P01。仰躺，雙臂放身體兩側，雙腿自然伸直。
    P01: { label: '仰躺，雙臂放在身體兩側，雙腿自然伸直', arm: [10, 5], leg: [5, 4] },
    // 來源：poses.md P02。仰躺，雙腳腳掌相對，兩膝向兩側打開（像翻開一本書）；雙手放身上或身體兩側。
    // 從上方看，膝蓋是抬起來的，所以大腿的投影比較短（legLen）；腳在中線相碰，呈菱形。
    P02: { label: '仰躺，雙腳腳掌相對，兩膝向兩側打開', arm: [18, 10], leg: [58, -100], legLen: [34, 36] }
  };

  function el(name, attrs, parent) {
    var n = document.createElementNS(NS, name);
    Object.keys(attrs).forEach(function (k) { n.setAttribute(k, attrs[k]); });
    if (parent) parent.appendChild(n);
    return n;
  }
  function rad(d) { return d * Math.PI / 180; }
  // 從 p0 依序接兩段，回傳 [p0, 關節, 末端]。side：1 為下半側，-1 為上半側（鏡射）
  function chain(p0, lens, angles, side) {
    var p1 = [p0[0] + lens[0] * Math.cos(rad(angles[0])), p0[1] + side * lens[0] * Math.sin(rad(angles[0]))];
    var p2 = [p1[0] + lens[1] * Math.cos(rad(angles[1])), p1[1] + side * lens[1] * Math.sin(rad(angles[1]))];
    return [p0, p1, p2];
  }
  function pts(c) { return c.map(function (p) { return p[0].toFixed(1) + ',' + p[1].toFixed(1); }).join(' '); }

  function makePoseIcon(id) {
    var P = POSES[id];
    if (!P) return null;
    var svg = el('svg', { viewBox: '0 0 184 120', role: 'img', 'aria-label': P.label, 'class': 'pose-icon' });
    el('rect', { 'class': 'ic-bed', x: 4, y: 4, width: 176, height: 112, rx: 14 }, svg);
    el('rect', { 'class': 'ic-pillow', x: 13, y: 38, width: 46, height: 44, rx: 12 }, svg);

    var legs = [1, -1].map(function (s) { return chain([G.hip.x, 60 + s * G.hip.dy], P.legLen || G.leg, P.leg, s); });
    var arms = [1, -1].map(function (s) { return chain([G.shoulder.x, 60 + s * G.shoulder.dy], G.arm, P.arm, s); });

    // 先畫腿、再畫手臂，每一肢都先描一圈背景色，讓肢體之間分得開
    legs.concat(arms).forEach(function (c, i) {
      var isArm = i >= legs.length;
      el('polyline', { 'class': 'ic-out', points: pts(c), 'stroke-width': G.limb + 4 }, svg);
      el('polyline', { 'class': 'ic-body', points: pts(c), 'stroke-width': G.limb }, svg);
      var end = c[2];
      el('circle', { 'class': 'ic-dot', cx: end[0].toFixed(1), cy: end[1].toFixed(1), r: isArm ? G.hand : G.foot }, svg);
    });

    el('line', { 'class': 'ic-body', x1: G.torso.x1, y1: 60, x2: G.torso.x2, y2: 60, 'stroke-width': G.torso.w }, svg);
    el('circle', { 'class': 'ic-dot', cx: G.head.x, cy: 60, r: G.head.r }, svg);
    return svg;
  }

  window.makePoseIcon = makePoseIcon;
  window.NIGHT_YOGA_ICON_IDS = Object.keys(POSES);
})();
