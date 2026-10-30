#!/usr/bin/env python3
"""用 Google AI Studio 的 Gemini 語音合成，把 script.csv 的每一句產生成語音檔（mp3，網頁用）。

用法：
  export GEMINI_API_KEY=你的金鑰        （到 https://aistudio.google.com 的 Get API key 申請）
  python3 tools/voice/generate.py        （在 repo 根目錄執行；需要安裝 ffmpeg 轉成 mp3）

輸出到 public/voice/<id>.mp3。已經有的檔案預設略過，要重做加 --force；
只做某幾句加 --only phase_inhale end。產生後請自己聽過一遍，不滿意的刪掉重跑（每次聽起來會有點不同）。

注意：這支腳本照 Google 文件的 Interactions API 寫成，作者環境連不到 Google，沒有實際執行過。
如果 API 格式改了或回傳不同，錯誤訊息會把回應印出來，照著調整 build_body() 與 find_audio() 即可。
只用 Python 標準函式庫，不需要安裝任何套件。
"""
import argparse
import array
import base64
import csv
import io
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
import wave

HERE = os.path.dirname(os.path.abspath(__file__))
API_ROOT = "https://generativelanguage.googleapis.com/v1beta"
ENDPOINT = API_ROOT + "/interactions"
DEFAULT_MODEL = "gemini-3.8-flash-tts"
CLASSIC_MODEL = "gemini-2.5-flash-preview-tts"  # --api classic 用的模型（generateContent 格式，語音合成最早公開的那一種）
DEFAULT_VOICE = "Zephyr"  # 明亮親切；睡前課程也可以試比較柔和的 Achernar、Sulafat、Vindemiatrix
SAMPLE_RATE = 24000

BASE_STYLE = "a warm, friendly and natural yoga teacher guiding a bedtime session, speaking Taiwanese Mandarin with natural, lively intonation, relaxed and never monotone"
STYLES = [
    # (id 前綴, 這類話的語氣)
    ("pose_", "gentle and reassuring with a soft smile in the voice, unhurried, like talking to a friend"),
    ("breath_", "calm, soft and inviting, unhurried"),
    ("phase_", "very soft, slow and soothing, a single guiding word"),
    ("end", "warm, tender and sleepy, a gentle goodnight with a smile"),
]


def style_for(clip_id: str) -> str:
    for prefix, style in STYLES:
        if clip_id.startswith(prefix):
            return f"{BASE_STYLE}; {style}"
    return BASE_STYLE


def build_body(model: str, voice: str, text: str, style: str) -> dict:
    return {
        "model": model,
        "input": [
            {
                "type": "user_input",
                "content": [
                    {
                        "type": "text",
                        "text": text,
                        "annotations": [{"type": "speech_metadata", "style": style}],
                    }
                ],
            }
        ],
        "response_format": {"type": "audio"},
        "generation_config": {"speech_config": [{"voice": voice}]},
    }


def build_body_classic(voice: str, text: str, style: str) -> dict:
    """generateContent 格式：語氣用一句英文指示放在文字前面。"""
    return {
        "contents": [{"parts": [{"text": f"Say this in {style}: {text}"}]}],
        "generationConfig": {
            "responseModalities": ["AUDIO"],
            "speechConfig": {"voiceConfig": {"prebuiltVoiceConfig": {"voiceName": voice}}},
        },
    }


def find_audio(node):
    """在回應裡找出音訊的 base64：Interactions 是 type 為 audio 的 data，generateContent 是 inlineData.data。"""
    if isinstance(node, dict):
        if node.get("type") == "audio" and isinstance(node.get("data"), str):
            return node["data"]
        inline = node.get("inlineData")
        if isinstance(inline, dict) and isinstance(inline.get("data"), str):
            return inline["data"]
        for value in node.values():
            found = find_audio(value)
            if found:
                return found
    elif isinstance(node, list):
        for value in node:
            found = find_audio(value)
            if found:
                return found
    return None


def request_audio(api_key: str, endpoint: str, body: dict, retries: int = 4) -> bytes:
    data = json.dumps(body).encode("utf-8")
    delay = 5
    for attempt in range(retries + 1):
        req = urllib.request.Request(
            endpoint,
            data=data,
            headers={"Content-Type": "application/json", "x-goog-api-key": api_key},
        )
        try:
            with urllib.request.urlopen(req, timeout=120) as resp:
                payload = json.loads(resp.read().decode("utf-8"))
        except urllib.error.HTTPError as e:
            detail = e.read().decode("utf-8", errors="replace")
            if e.code in (429, 500, 503) and attempt < retries:
                print(f"    HTTP {e.code}，{delay} 秒後重試…", file=sys.stderr)
                time.sleep(delay)
                delay *= 2
                continue
            raise SystemExit(f"API 錯誤 HTTP {e.code}：{detail[:800]}")
        b64 = find_audio(payload)
        if not b64:
            raise SystemExit("回應裡找不到音訊，回應內容：\n" + json.dumps(payload, ensure_ascii=False)[:1500])
        return base64.b64decode(b64)
    raise SystemExit("重試多次仍失敗")


def to_pcm(raw: bytes):
    """回傳（16 位元單聲道 PCM 樣本, 取樣率）。回應通常是帶標頭的 wav，沒有標頭就當作 24 kHz 的原始 PCM。"""
    if raw[:4] == b"RIFF":
        with wave.open(io.BytesIO(raw), "rb") as w:
            if w.getsampwidth() != 2:
                raise SystemExit("不是 16 位元音訊")
            frames = w.readframes(w.getnframes())
            samples = array.array("h")
            samples.frombytes(frames)
            if sys.byteorder == "big":
                samples.byteswap()
            if w.getnchannels() == 2:
                samples = array.array("h", samples[::2])
            return samples, w.getframerate()
    samples = array.array("h")
    samples.frombytes(raw[: len(raw) // 2 * 2])
    if sys.byteorder == "big":
        samples.byteswap()
    return samples, SAMPLE_RATE


def polish(samples, rate: int, lead_ms: int = 60, tail_ms: int = 160):
    """切掉前後的靜音（讀秒才不會慢半拍），並把音量統一到接近滿格。"""
    peak = max((abs(x) for x in samples), default=0)
    if peak == 0:
        return samples
    threshold = peak * 0.02
    first = next((i for i, x in enumerate(samples) if abs(x) > threshold), 0)
    last = next((i for i in range(len(samples) - 1, -1, -1) if abs(samples[i]) > threshold), len(samples) - 1)
    start = max(0, first - rate * lead_ms // 1000)
    end = min(len(samples), last + 1 + rate * tail_ms // 1000)
    trimmed = samples[start:end]
    gain = 0.89 * 32767 / peak
    return array.array("h", (max(-32768, min(32767, int(x * gain))) for x in trimmed))


def write_wav(path: str, samples, rate: int):
    out = array.array("h", samples)
    if sys.byteorder == "big":
        out.byteswap()
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(out.tobytes())


def write_mp3(path: str, samples, rate: int):
    """先寫成暫存 wav，再用 ffmpeg 轉成單聲道 mp3（網頁才不會太大）。"""
    tmp = path + ".tmp.wav"
    write_wav(tmp, samples, rate)
    try:
        subprocess.run(
            ["ffmpeg", "-y", "-loglevel", "error", "-i", tmp, "-ac", "1", "-codec:a", "libmp3lame", "-b:a", "64k", path],
            check=True,
        )
    except FileNotFoundError:
        raise SystemExit("找不到 ffmpeg，請先安裝（例如 sudo apt install ffmpeg 或 brew install ffmpeg）")
    finally:
        if os.path.exists(tmp):
            os.remove(tmp)


def main():
    parser = argparse.ArgumentParser(description="用 Gemini 語音合成產生 晚安瑜伽 的語音檔")
    parser.add_argument("--script", default=os.path.join(HERE, "script.csv"))
    parser.add_argument("--out", default=os.path.normpath(os.path.join(HERE, "..", "..", "public", "voice")))
    parser.add_argument("--api", choices=["interactions", "classic"], default="interactions",
                        help="API 格式。預設 interactions；若回應格式不對，改用 classic（generateContent）")
    parser.add_argument("--model", help=f"模型名稱（預設 interactions 用 {DEFAULT_MODEL}，classic 用 {CLASSIC_MODEL}）")
    parser.add_argument("--voice", default=DEFAULT_VOICE)
    parser.add_argument("--only", nargs="*", help="只產生這些 id")
    parser.add_argument("--force", action="store_true", help="已有的檔案也重做")
    parser.add_argument("--no-polish", action="store_true", help="不要切靜音與調音量")
    parser.add_argument("--pause", type=float, default=1.0, help="每句之間等幾秒（避免超過速率限制）")
    args = parser.parse_args()

    api_key = os.environ.get("GEMINI_API_KEY") or os.environ.get("GOOGLE_API_KEY")
    if not api_key:
        raise SystemExit("請先設定環境變數 GEMINI_API_KEY（到 https://aistudio.google.com 的 Get API key 申請）")

    with open(args.script, encoding="utf-8", newline="") as f:
        rows = list(csv.DictReader(f))
    os.makedirs(args.out, exist_ok=True)

    todo = [r for r in rows if not args.only or r["id"] in args.only]
    made = skipped = 0
    for n, row in enumerate(todo, 1):
        path = os.path.join(args.out, row["id"] + ".mp3")
        if os.path.exists(path) and not args.force:
            skipped += 1
            continue
        print(f"[{n}/{len(todo)}] {row['id']}：{row['text']}")
        if args.api == "classic":
            endpoint = f"{API_ROOT}/models/{args.model or CLASSIC_MODEL}:generateContent"
            body = build_body_classic(args.voice, row["text"], style_for(row["id"]))
        else:
            endpoint = ENDPOINT
            body = build_body(args.model or DEFAULT_MODEL, args.voice, row["text"], style_for(row["id"]))
        raw = request_audio(api_key, endpoint, body)
        samples, rate = to_pcm(raw)
        if not args.no_polish:
            samples = polish(samples, rate)
        write_mp3(path, samples, rate)
        print(f"    → {path}（{len(samples) / rate:.1f} 秒）")
        made += 1
        time.sleep(args.pause)
    print(f"完成：新產生 {made} 句，略過已有的 {skipped} 句。請逐句聽過，不滿意的刪掉重跑。")


if __name__ == "__main__":
    main()
