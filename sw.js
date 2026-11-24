// 離線快取：
// - 頁面導覽先走網路（拿新版 HTML），離線才退回快取。
// - 帶內容雜湊的建置檔（.wasm / .js / .css）與字型檔走 Cache-First，並自動清掉舊雜湊檔。
// - 示範圖、預錄語音與圖示沒有檔名雜湊，走 Stale-While-Revalidate（秒讀快取、背景更新）。
// - 第三方請求（除 Google Fonts 外）一律不攔截，避免誤快取外部 API。
const CACHE = "night-yoga-v9";
const HASHED_RE = /^(.*)-[0-9a-f]{16}(_bg)?\.(wasm|js|css)$/;

function canCacheResponse(res, allowOpaque) {
  return res && (res.status === 200 || (allowOpaque && res.type === "opaque"));
}

async function pruneOldHashed(cache, pathname, match) {
  const [, prefix, bg = "", ext] = match;
  const suffix = `${bg}.${ext}`;
  const keys = await cache.keys();
  await Promise.all(
    keys.map((req) => {
      const p = new URL(req.url).pathname;
      if (p !== pathname && HASHED_RE.test(p) && p.startsWith(`${prefix}-`) && p.endsWith(suffix)) {
        return cache.delete(req);
      }
      return undefined;
    })
  );
}

self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (e) => {
  e.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k))))
      .then(() => self.clients.claim())
  );
});

self.addEventListener("fetch", (e) => {
  const req = e.request;
  if (req.method !== "GET") return;

  const url = new URL(req.url);
  const sameOrigin = url.origin === self.location.origin;
  const isFontCss = url.hostname === "fonts.googleapis.com";
  const isFontFile = url.hostname === "fonts.gstatic.com";
  if (!sameOrigin && !isFontCss && !isFontFile) return;

  if (req.mode === "navigate") {
    e.respondWith(
      fetch(req)
        .then((res) => {
          if (res.status === 200) {
            const copy = res.clone();
            caches.open(CACHE).then((c) => c.put(req, copy));
          }
          return res;
        })
        .catch(() => caches.match(req).then((r) => r || caches.match("./")))
    );
    return;
  }

  // Safari 播放 <audio> 會送 Range 請求（回傳 206 Partial Content，不能直接放進 Cache API）：
  // 若快取裡已經有預載好的完整 200 回應就直接回傳，否則交給網路處理。
  if (req.headers.has("range")) {
    e.respondWith(caches.match(req.url).then((hit) => hit || fetch(req)));
    return;
  }

  const hashedMatch = sameOrigin ? url.pathname.match(HASHED_RE) : null;
  if (hashedMatch || isFontFile) {
    e.respondWith(
      caches.match(req).then(
        (hit) =>
          hit ||
          fetch(req).then((res) => {
            if (canCacheResponse(res, isFontFile)) {
              const copy = res.clone();
              e.waitUntil(
                caches.open(CACHE).then(async (c) => {
                  await c.put(req, copy);
                  if (hashedMatch) {
                    await pruneOldHashed(c, url.pathname, hashedMatch);
                  }
                })
              );
            }
            return res;
          })
      )
    );
    return;
  }

  // Stale-While-Revalidate：有快取先回傳，同時在背景抓新版更新快取
  e.respondWith(
    caches.match(req).then((hit) => {
      const network = fetch(req)
        .then((res) => {
          if (canCacheResponse(res, isFontCss)) {
            const copy = res.clone();
            caches.open(CACHE).then((c) => c.put(req, copy));
          }
          return res;
        })
        .catch((err) => {
          if (hit) return hit;
          throw err;
        });
      if (hit) {
        e.waitUntil(network.catch(() => {}));
        return hit;
      }
      return network;
    })
  );
});
