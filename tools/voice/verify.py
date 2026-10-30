#!/usr/bin/env python3
"""檢查 generate.py 產生的語音檔品質，產出 report.csv、report.json 與可以直接聽的 review.html。

用法（在 repo 根目錄執行）：
  python tools/voice/verify.py                 只做訊號檢查（免費、離線，需要 ffmpeg / ffprobe）
  python tools/voice/verify.py --listen        再請 Gemini 聽每一句：轉成文字比對語音稿，並評分
                                               （需要環境變數 GEMINI_API_KEY）
  python tools/voice/verify.py --only count_5  只檢查幾句

三層檢查：
  A 訊號：格式、長度、語速、前後靜音、中間停頓、音量一致、爆音、音高起伏。
  B 內容：Gemini 在「不知道語音稿」的情況下轉寫，再跟 script.csv 算字錯率，抓漏念、多念、念錯。
  C 評審：Gemini 對台灣腔、風格貼合、自然度、清晰度打 1~5 分並列出問題。
B、C 是同一次呼叫的結果，會依檔案內容快取在報告資料夾的 cache.json，重跑不會重複花錢。

限制：C 是模型評分，只拿來篩出可疑的句子，最後仍要用耳朵聽；review.html 就是為了讓你只聽有紅旗的和抽樣。
只用 Python 標準函式庫。
"""
import argparse
import base64
import csv
import hashlib
import html
import json
import math
import os
import re
import shutil
import statistics
import subprocess
import sys
import time
import unicodedata
import urllib.error
import urllib.request
import wave
import array

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
DEFAULT_JUDGE_MODEL = "gemini-3.8-flash"
API_ROOT = "https://generativelanguage.googleapis.com/v1beta"

# 各類語音「預期的風格」，只用來告訴評審該怎麼打風格貼合度
STYLE_HINTS = {
    "count_": "crisp, punchy, upbeat one-word countdown shout (workout class)",
    "coach_": "crisp, energetic coach cue",
    "cheer_": "warm, excited, genuinely encouraging with a smile in the voice",
    "move_": "clear, lively, friendly, demonstrating an exercise move",
    "stage_": "bright, energetic, announcing the start of the next workout part",
    "elapsed_": "upbeat and proud quick progress update",
    "plan_done": "very happy, proud, celebratory congratulation",
    "pose_": "gentle, reassuring, unhurried yoga teacher guiding a bedtime pose",
    "breath_": "calm, soft, inviting, unhurried breathing instruction",
    "phase_": "very soft, slow, soothing single guiding word",
    "end": "warm, tender, sleepy goodnight with a smile",
}
# 這幾類不要求語氣起伏大（短促口令、單字）
FLAT_OK = ("count_", "phase_")

CJK = re.compile(r"[一-鿿]")
DIGITS = "零一二三四五六七八九"


def style_hint(clip_id):
    for prefix, hint in STYLE_HINTS.items():
        if clip_id.startswith(prefix):
            return hint
    return "natural, friendly Taiwanese Mandarin"


def group_of(clip_id):
    for prefix in STYLE_HINTS:
        if clip_id.startswith(prefix):
            return prefix
    return clip_id


# ---------- 文字正規化與字錯率 ----------

def cn_number(n):
    if n < 10:
        return DIGITS[n]
    if n < 20:
        return "十" + (DIGITS[n - 10] if n > 10 else "")
    if n < 100:
        return DIGITS[n // 10] + "十" + (DIGITS[n % 10] if n % 10 else "")
    return "".join(DIGITS[int(c)] for c in str(n))


def normalize(text):
    s = unicodedata.normalize("NFKC", text)
    s = re.sub(r"\d+(?:-\d+)+", lambda m: "".join(DIGITS[int(c)] for c in m.group(0) if c.isdigit()), s)
    s = re.sub(r"\d+", lambda m: cn_number(int(m.group(0))), s)
    s = s.replace("兩", "二")
    return "".join(c for c in s if CJK.match(c) or c.isalnum()).lower()


def edit_distance(a, b):
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (ca != cb)))
        prev = cur
    return prev[-1]


def char_error_rate(expected, heard):
    e, h = normalize(expected), normalize(heard)
    if not e:
        return 0.0
    return edit_distance(e, h) / len(e)


# ---------- A 層：訊號 ----------

def run(cmd):
    return subprocess.run(cmd, capture_output=True, check=False)


def probe(path):
    r = run(["ffprobe", "-v", "error", "-select_streams", "a:0", "-show_entries",
             "stream=codec_name,sample_rate,channels,bits_per_sample", "-of", "json", path])
    try:
        return json.loads(r.stdout)["streams"][0]
    except (ValueError, KeyError, IndexError):
        return None


def decode(path, rate=16000):
    r = run(["ffmpeg", "-v", "error", "-i", path, "-f", "s16le", "-ac", "1", "-ar", str(rate), "-"])
    samples = array.array("h")
    samples.frombytes(r.stdout[: len(r.stdout) // 2 * 2])
    if sys.byteorder == "big":
        samples.byteswap()
    return samples


def db(x):
    return 20 * math.log10(x) if x > 0 else -120.0


def frame_rms(samples, rate, ms=20):
    n = rate * ms // 1000
    out = []
    for i in range(0, len(samples) - n + 1, n):
        seg = samples[i:i + n]
        out.append(math.sqrt(sum(x * x for x in seg) / n))
    return out


def estimate_pitch(samples, rate=16000):
    """粗略的音高變化（半音標準差）。降到 8 kHz 後做自相關，只用來看「是不是平到像機器」。"""
    low = array.array("h", ((samples[i] + samples[i + 1]) // 2 for i in range(0, len(samples) - 1, 2)))
    r = rate // 2
    frame, hop = int(r * 0.04), int(r * 0.03)
    lo, hi = r // 400, r // 70
    peak = max((abs(x) for x in low), default=1) or 1
    f0s = []
    for start in range(0, len(low) - frame - hi, hop):
        seg = low[start:start + frame]
        energy = sum(x * x for x in seg)
        if math.sqrt(energy / frame) < peak * 0.05:
            continue
        best_lag, best = 0, 0.0
        for lag in range(lo, hi):
            c = sum(seg[i] * low[start + i + lag] for i in range(frame))
            if c > best:
                best, best_lag = c, lag
        if best_lag and best / energy > 0.5:
            f0s.append(r / best_lag)
    if len(f0s) < 5:
        return None, len(f0s)
    med = statistics.median(f0s)
    st = [12 * math.log2(f / med) for f in f0s]
    return statistics.pstdev(st), len(f0s)


def analyze_signal(path, text):
    m = {"flags": [], "errors": []}
    info = probe(path)
    if not info:
        m["errors"].append("無法讀取音訊")
        return m
    m["codec"], m["rate"], m["channels"] = info.get("codec_name"), int(info.get("sample_rate") or 0), int(info.get("channels") or 0)
    if m["channels"] != 1:
        m["errors"].append(f"不是單聲道（{m['channels']}）")
    if path.endswith(".wav") and (m["rate"] != 24000 or str(info.get("bits_per_sample")) not in ("16", "0")):
        m["errors"].append(f"wav 規格不是 24 kHz 16 位元（{m['rate']} Hz）")

    samples = decode(path)
    rate = 16000
    if len(samples) == 0:
        m["errors"].append("解碼後沒有聲音資料")
        return m
    m["duration"] = round(len(samples) / rate, 2)
    peak = max(abs(x) for x in samples)
    m["peak_db"] = round(db(peak / 32768), 1)
    if peak < 32768 * 0.01:
        m["errors"].append("幾乎是靜音")
        return m
    clipped = sum(1 for x in samples if abs(x) >= 32700)
    m["clipped"] = clipped
    if clipped / len(samples) > 0.0005:
        m["errors"].append(f"爆音（{clipped} 個樣本）")

    rms = frame_rms(samples, rate)
    thr = peak * 0.02
    active = [v for v in rms if v > thr]
    m["rms_db"] = round(db(statistics.fmean(active) / 32768), 1) if active else -120.0
    first = next((i for i, x in enumerate(samples) if abs(x) > thr), 0)
    last = next((i for i in range(len(samples) - 1, -1, -1) if abs(samples[i]) > thr), len(samples) - 1)
    m["lead_ms"] = round(first * 1000 / rate)
    m["tail_ms"] = round((len(samples) - 1 - last) * 1000 / rate)
    if m["lead_ms"] < 20:
        m["flags"].append("開頭太貼近聲音，可能被切到")
    if m["tail_ms"] < 80:
        m["flags"].append("結尾太貼近聲音，可能被切到")
    if m["lead_ms"] > 400:
        m["flags"].append(f"開頭靜音 {m['lead_ms']} ms（讀秒會慢半拍）")
    if m["tail_ms"] > 700:
        m["flags"].append(f"結尾靜音 {m['tail_ms']} ms")

    # 中間最長停頓
    quiet = peak * 0.01
    longest = run_len = 0
    body = rms[first // (rate * 20 // 1000): max(1, (last // (rate * 20 // 1000)) + 1)]
    for v in body:
        run_len = run_len + 1 if v < quiet else 0
        longest = max(longest, run_len)
    m["max_pause_ms"] = longest * 20
    if longest * 20 > 1200:
        m["flags"].append(f"中間停頓 {longest * 20} ms")

    chars = len(CJK.findall(normalize(text))) or len(normalize(text))
    speech = (last - first) / rate
    m["chars"] = chars
    m["rate_cps"] = round(chars / speech, 2) if speech > 0 else 0
    if chars >= 6 and not 2.0 <= m["rate_cps"] <= 8.5:
        m["errors"].append(f"語速異常 {m['rate_cps']} 字/秒（可能被截斷、拖長或亂念）")

    if len(samples) / rate >= 1.2 and not any(text_id_is_flat_ok(path)):
        sd, voiced = estimate_pitch(samples)
        m["pitch_sd_st"] = round(sd, 2) if sd is not None else None
        if sd is not None and sd < 1.2:
            m["flags"].append(f"音高起伏很小（{sd:.1f} 半音），可能聽起來平板")
    return m


def text_id_is_flat_ok(path):
    base = os.path.basename(path)
    return [base.startswith(p) for p in FLAT_OK]


def add_relative_flags(rows):
    """同一類的語速、音量互相比，離群的標警告。"""
    groups = {}
    for r in rows:
        groups.setdefault(group_of(r["id"]), []).append(r)
    rms_all = [r["signal"]["rms_db"] for r in rows if "rms_db" in r["signal"]]
    rms_med = statistics.median(rms_all) if rms_all else 0
    for g, items in groups.items():
        rates = [i["signal"]["rate_cps"] for i in items if i["signal"].get("rate_cps") and i["signal"].get("chars", 0) >= 4]
        if len(rates) >= 3:
            med = statistics.median(rates)
            for i in items:
                v = i["signal"].get("rate_cps")
                if v and i["signal"].get("chars", 0) >= 4 and not 0.6 <= v / med <= 1.6:
                    i["signal"]["flags"].append(f"語速 {v} 字/秒，同類中位數 {med:.1f}")
    for r in rows:
        v = r["signal"].get("rms_db")
        if v is not None and abs(v - rms_med) > 6:
            r["signal"]["flags"].append(f"音量 {v} dB，整體中位數 {rms_med:.1f} dB")


# ---------- B、C 層：請 Gemini 聽 ----------

PROMPT = """You are a careful listener evaluating one short voice clip made by a text-to-speech system for a Taiwanese Mandarin fitness / yoga app.
Intended style of this clip: {style}.

Listen to the audio, then return ONLY a JSON object with these keys:
- "transcript": exactly what you hear, in Traditional Chinese characters (繁體中文), with numbers written as Chinese characters. Do not guess what the speaker was supposed to say; if words are missing, repeated, slurred or mispronounced, write what is actually audible.
- "taiwan_accent": 1-5. 5 = sounds like a native speaker of Taiwanese Mandarin (台灣國語: no 兒化音, Taiwanese-style neutral tones and rhythm). 1 = clearly mainland / other regional / foreign-sounding.
- "style_fit": 1-5, how well the energy, pace and emotion match the intended style above.
- "naturalness": 1-5, 5 = indistinguishable from a human, 1 = robotic or glitchy.
- "clarity": 1-5, 5 = every syllable crisp, 1 = mumbled or distorted.
- "mainland_features": list of short Chinese notes about specific mainland-sounding features you hear (empty list if none).
- "issues": list of short Chinese notes about any problems (mispronounced characters, wrong tone, cut-off, glitch, odd pause, noise). Empty list if none.
- "verdict": "ok" if you would ship this clip, otherwise "redo".
Be strict and honest; do not give 5 by default."""


def gemini_listen(api_key, model, path, style, retries=4):
    mime = "audio/mpeg" if path.endswith(".mp3") else "audio/wav"
    with open(path, "rb") as f:
        data = base64.b64encode(f.read()).decode("ascii")
    body = {
        "contents": [{"parts": [{"text": PROMPT.format(style=style)}, {"inline_data": {"mime_type": mime, "data": data}}]}],
        "generationConfig": {"temperature": 0, "responseMimeType": "application/json"},
    }
    url = f"{API_ROOT}/models/{model}:generateContent"
    payload = json.dumps(body).encode("utf-8")
    delay = 5
    for attempt in range(retries + 1):
        req = urllib.request.Request(url, data=payload, headers={"Content-Type": "application/json", "x-goog-api-key": api_key})
        try:
            with urllib.request.urlopen(req, timeout=120) as resp:
                out = json.loads(resp.read().decode("utf-8"))
            text = "".join(p.get("text", "") for p in out["candidates"][0]["content"]["parts"])
            text = re.sub(r"^```(?:json)?|```$", "", text.strip(), flags=re.M).strip()
            return json.loads(text)
        except urllib.error.HTTPError as e:
            detail = e.read().decode("utf-8", errors="replace")
            if "spending cap" in detail or "billing" in detail.lower():
                raise SystemExit("Gemini 專案已達額度上限，無法繼續：\n" + detail[:400])
            if e.code in (429, 500, 503) and attempt < retries:
                print(f"    HTTP {e.code}，{delay} 秒後重試…", file=sys.stderr)
                time.sleep(delay)
                delay *= 2
                continue
            return {"error": f"HTTP {e.code}：{detail[:300]}"}
        except (KeyError, IndexError, ValueError) as e:
            if attempt < retries:
                time.sleep(2)
                continue
            return {"error": f"回應解析失敗：{e}"}
    return {"error": "重試多次仍失敗"}


def judge_flags(listen, text):
    errors, flags = [], []
    if not listen or "error" in listen:
        return errors, [f"Gemini 失敗：{listen.get('error') if listen else '無回應'}"] if listen is not None else []
    cer = char_error_rate(text, listen.get("transcript", ""))
    listen["cer"] = round(cer, 3)
    if cer > 0.15 or (len(normalize(text)) <= 3 and cer > 0):
        errors.append(f"轉寫與語音稿不符（字錯率 {cer:.0%}）：聽到「{listen.get('transcript', '')}」")
    low = [label for k, label in (("taiwan_accent", "台灣腔"), ("style_fit", "風格"), ("naturalness", "自然度"), ("clarity", "清晰度"))
           if isinstance(listen.get(k), (int, float)) and listen[k] <= 2]
    if low:
        flags.append("評審給低分：" + "、".join(low))
    if listen.get("verdict") == "redo":
        flags.append("評審建議重做")
    for note in listen.get("issues") or []:
        flags.append("評審：" + str(note))
    return errors, flags


# ---------- 輸出 ----------

def find_voice_dir():
    for rel in (("app", "src", "main", "assets", "voice"), ("public", "voice")):
        p = os.path.join(ROOT, *rel)
        if os.path.isdir(p):
            return p
    raise SystemExit("找不到語音資料夾，請用 --dir 指定")


def write_reports(rows, out_dir, voice_dir, judge_model):
    os.makedirs(out_dir, exist_ok=True)
    with open(os.path.join(out_dir, "report.json"), "w", encoding="utf-8") as f:
        json.dump(rows, f, ensure_ascii=False, indent=1)
    cols = ["id", "text", "status", "duration", "rate_cps", "lead_ms", "tail_ms", "max_pause_ms", "peak_db", "rms_db", "pitch_sd_st",
            "cer", "transcript", "taiwan_accent", "style_fit", "naturalness", "clarity", "verdict", "problems"]
    with open(os.path.join(out_dir, "report.csv"), "w", encoding="utf-8-sig", newline="") as f:
        w = csv.writer(f)
        w.writerow(cols)
        for r in rows:
            s, l = r["signal"], r.get("listen") or {}
            flat = {**s, **{k: l.get(k) for k in ("cer", "transcript", "taiwan_accent", "style_fit", "naturalness", "clarity", "verdict")}}
            w.writerow([r["id"], r["text"], r["status"]] + [flat.get(c, "") for c in cols[3:-1]] + [" ／ ".join(r["problems"])])

    rel = os.path.relpath(voice_dir, out_dir).replace(os.sep, "/")
    page = REVIEW_TEMPLATE.replace("__DATA__", json.dumps({"rows": rows, "audioDir": rel, "model": judge_model}, ensure_ascii=False).replace("</", "<\\/"))
    with open(os.path.join(out_dir, "review.html"), "w", encoding="utf-8") as f:
        f.write(page)


REVIEW_TEMPLATE = """<!doctype html>
<html lang="zh-Hant"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>語音驗收</title>
<style>
:root{--bg:#fff;--fg:#1c1c1e;--mut:#6b7280;--line:#e5e7eb;--err:#dc2626;--warn:#d97706;--ok:#16a34a;--card:#f8fafc}
@media (prefers-color-scheme:dark){:root{--bg:#141416;--fg:#ececf0;--mut:#9ca3af;--line:#2d2d33;--card:#1c1c20}}
body{margin:0;background:var(--bg);color:var(--fg);font:15px/1.5 system-ui,"Noto Sans TC",sans-serif}
main{max-width:980px;margin:0 auto;padding:16px}
h1{font-size:20px;margin:8px 0}.sum{color:var(--mut)}
.bar{position:sticky;top:0;background:var(--bg);padding:8px 0;border-bottom:1px solid var(--line);display:flex;gap:8px;flex-wrap:wrap;align-items:center;z-index:2}
button,select{font:inherit;padding:4px 10px;border:1px solid var(--line);border-radius:6px;background:var(--card);color:var(--fg);cursor:pointer}
.row{border:1px solid var(--line);border-radius:8px;padding:10px 12px;margin:10px 0;background:var(--card)}
.row.error{border-left:4px solid var(--err)}.row.warn{border-left:4px solid var(--warn)}.row.ok{border-left:4px solid var(--ok)}
.id{font-weight:600}.txt{margin:2px 0 6px}.m{color:var(--mut);font-size:13px}
.pb{margin:4px 0;font-size:13px}.pb.e{color:var(--err)}.pb.w{color:var(--warn)}
audio{width:100%;max-width:420px;height:34px}
.verdict button.on{outline:2px solid var(--fg)}
textarea{width:100%;box-sizing:border-box;min-height:60px;font:13px monospace;background:var(--card);color:var(--fg);border:1px solid var(--line);border-radius:6px}
</style></head><body><main>
<h1>語音驗收</h1><div class="sum" id="sum"></div>
<div class="bar"><select id="filter"><option value="flagged">只看有紅旗的</option><option value="all">全部</option><option value="error">只看嚴重</option><option value="redo">只看我標重做的</option></select>
<button id="export">匯出重做清單</button><span class="m" id="cnt"></span></div>
<div id="list"></div><div id="out" hidden><p>重做指令（在 repo 根目錄執行；先用 --force 讓它覆蓋）：</p><textarea id="cmd" readonly></textarea></div>
</main><script>
const D=__DATA__;const KEY="voice-review:"+location.pathname;
let mark={};try{mark=JSON.parse(localStorage.getItem(KEY)||"{}")}catch(e){}
const save=()=>{try{localStorage.setItem(KEY,JSON.stringify(mark))}catch(e){}};
const esc=s=>String(s??"").replace(/[&<>"]/g,c=>({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;"}[c]));
const n=D.rows.length,e=D.rows.filter(r=>r.status=="error").length,w=D.rows.filter(r=>r.status=="warn").length;
document.getElementById("sum").textContent=`共 ${n} 句：嚴重 ${e}、警告 ${w}、正常 ${n-e-w}。`+(D.rows.some(r=>r.listen)?`（含 Gemini ${D.model} 聽寫與評分；評分只是篩選用，請以你的耳朵為準）`:"（只有訊號檢查；加 --listen 才會比對內容與評分）");
function metrics(r){const s=r.signal,l=r.listen||{};const p=[];
 if(s.duration!=null)p.push(`${s.duration}s`);if(s.rate_cps)p.push(`${s.rate_cps}字/秒`);if(s.rms_db!=null)p.push(`音量${s.rms_db}dB`);
 if(s.pitch_sd_st!=null)p.push(`起伏${s.pitch_sd_st}st`);
 if(l.transcript!=null)p.push(`聽到「${esc(l.transcript)}」(字錯率${Math.round((l.cer||0)*100)}%)`);
 if(l.taiwan_accent!=null)p.push(`台灣腔${l.taiwan_accent}／風格${l.style_fit}／自然${l.naturalness}／清晰${l.clarity}`);
 return p.join("　·　")}
function render(){const f=document.getElementById("filter").value;const box=document.getElementById("list");box.innerHTML="";let shown=0;
 const order={error:0,warn:1,ok:2};
 [...D.rows].sort((a,b)=>order[a.status]-order[b.status]).forEach(r=>{
  if(f=="flagged"&&r.status=="ok")return;if(f=="error"&&r.status!="error")return;if(f=="redo"&&mark[r.id]!="redo")return;shown++;
  const d=document.createElement("div");d.className="row "+r.status;
  d.innerHTML=`<div><span class="id">${esc(r.id)}</span></div><div class="txt">${esc(r.text)}</div>
  <audio controls preload="none" src="${D.audioDir}/${encodeURIComponent(r.file)}"></audio>
  <div class="m">${metrics(r)}</div>
  ${r.errors.map(x=>`<div class="pb e">✖ ${esc(x)}</div>`).join("")}${r.flags.map(x=>`<div class="pb w">⚠ ${esc(x)}</div>`).join("")}
  <div class="verdict" style="margin-top:6px"><button data-v="pass" class="${mark[r.id]=="pass"?"on":""}">過</button> <button data-v="redo" class="${mark[r.id]=="redo"?"on":""}">重做</button></div>`;
  d.querySelectorAll("button").forEach(b=>b.onclick=()=>{mark[r.id]=mark[r.id]==b.dataset.v?undefined:b.dataset.v;save();render()});
  box.appendChild(d)});
 document.getElementById("cnt").textContent=`顯示 ${shown} 句；已標重做 ${Object.values(mark).filter(v=>v=="redo").length}`}
document.getElementById("filter").onchange=render;
document.getElementById("export").onclick=()=>{const ids=Object.keys(mark).filter(k=>mark[k]=="redo");
 document.getElementById("out").hidden=false;
 document.getElementById("cmd").value=ids.length?`python tools/voice/generate.py --force --only ${ids.join(" ")}`:"（還沒有標記重做的句子）"};
render();
</script></body></html>
"""


def main():
    ap = argparse.ArgumentParser(description="檢查語音檔品質")
    ap.add_argument("--dir", help="語音資料夾（預設自動判斷）")
    ap.add_argument("--script", default=os.path.join(HERE, "script.csv"))
    ap.add_argument("--out", default=os.path.join(HERE, "report"), help="報告資料夾")
    ap.add_argument("--only", nargs="*", help="只檢查這些 id")
    ap.add_argument("--listen", action="store_true", help="請 Gemini 轉寫並評分（需要 GEMINI_API_KEY）")
    ap.add_argument("--model", default=DEFAULT_JUDGE_MODEL, help=f"評審模型（預設 {DEFAULT_JUDGE_MODEL}）")
    ap.add_argument("--pause", type=float, default=0.5, help="每次呼叫 Gemini 之間等幾秒")
    args = ap.parse_args()
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")

    for tool in ("ffmpeg", "ffprobe"):
        if not shutil.which(tool):
            raise SystemExit(f"找不到 {tool}，請先安裝 ffmpeg")
    voice_dir = os.path.abspath(args.dir) if args.dir else find_voice_dir()
    with open(args.script, encoding="utf-8", newline="") as f:
        script = list(csv.DictReader(f))
    if args.only:
        script = [r for r in script if r["id"] in args.only]

    api_key = None
    cache_path = os.path.join(args.out, "cache.json")
    cache = {}
    if args.listen:
        api_key = os.environ.get("GEMINI_API_KEY") or os.environ.get("GOOGLE_API_KEY")
        if not api_key:
            raise SystemExit("--listen 需要環境變數 GEMINI_API_KEY")
        if os.path.exists(cache_path):
            with open(cache_path, encoding="utf-8") as f:
                cache = json.load(f)

    rows, missing = [], []
    for i, item in enumerate(script, 1):
        clip_id, text = item["id"], item["text"]
        file = next((clip_id + ext for ext in (".wav", ".mp3") if os.path.exists(os.path.join(voice_dir, clip_id + ext))), None)
        if not file:
            missing.append(clip_id)
            continue
        path = os.path.join(voice_dir, file)
        print(f"[{i}/{len(script)}] {clip_id}", flush=True)
        row = {"id": clip_id, "text": text, "file": file, "signal": analyze_signal(path, text), "listen": None}
        row["errors"] = list(row["signal"]["errors"])
        rows.append(row)

        if api_key and not row["signal"]["errors"]:
            with open(path, "rb") as f:
                digest = hashlib.sha1(f.read()).hexdigest() + ":" + args.model
            if digest not in cache:
                cache[digest] = gemini_listen(api_key, args.model, path, style_hint(clip_id))
                if "error" not in cache[digest]:
                    os.makedirs(args.out, exist_ok=True)
                    with open(cache_path, "w", encoding="utf-8") as f:
                        json.dump(cache, f, ensure_ascii=False)
                time.sleep(args.pause)
            row["listen"] = dict(cache[digest])

    add_relative_flags(rows)
    for r in rows:
        r["flags"] = list(r["signal"]["flags"])
        if r["listen"] is not None:
            errs, flags = judge_flags(r["listen"], r["text"])
            r["errors"] += errs
            r["flags"] += flags
        r["problems"] = r["errors"] + r["flags"]
        r["status"] = "error" if r["errors"] else "warn" if r["flags"] else "ok"

    extra = []
    if os.path.isdir(voice_dir) and not args.only:
        known = {r["id"] for r in script}
        extra = sorted(os.path.splitext(f)[0] for f in os.listdir(voice_dir) if f.endswith((".wav", ".mp3")) and os.path.splitext(f)[0] not in known)

    write_reports(rows, args.out, voice_dir, args.model)
    counts = {s: sum(1 for r in rows if r["status"] == s) for s in ("ok", "warn", "error")}
    print(f"\n檢查 {len(rows)} 句：正常 {counts['ok']}、警告 {counts['warn']}、嚴重 {counts['error']}")
    if missing:
        print(f"缺檔 {len(missing)} 句：{' '.join(missing)}")
    if extra:
        print(f"語音稿沒有的多餘檔案：{' '.join(extra)}")
    for r in rows:
        if r["status"] != "ok":
            print(f"  [{r['status']}] {r['id']}：{' ／ '.join(r['problems'])}")
    print(f"\n報告：{os.path.join(args.out, 'review.html')}（用瀏覽器開，可以直接聽並標記重做）")
    sys.exit(1 if counts["error"] or missing else 0)


if __name__ == "__main__":
    main()
