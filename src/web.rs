//! 瀏覽器 API：聲音、語音提示、螢幕常亮、localStorage。
//! 這些物件不是 Send，放在 thread_local（WASM 只有單一執行緒）。

use js_sys::{Function, Promise, Reflect};
use std::cell::{Cell, RefCell};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{AudioContext, HtmlAudioElement, OscillatorType, SpeechSynthesisUtterance};

thread_local! {
    static AUDIO: RefCell<Option<AudioContext>> = const { RefCell::new(None) };
    static WAKE: RefCell<Option<JsValue>> = const { RefCell::new(None) };
    static CLIP: RefCell<Option<HtmlAudioElement>> = const { RefCell::new(None) };
    /// 每念一句就加一，讓晚到的「播不出來」回呼知道這句已經過時。
    static SAY_SEQ: Cell<u32> = const { Cell::new(0) };
}

/// 整個 App 共用一個 AudioContext。
/// 瀏覽器規定要在使用者點擊後才能啟動聲音，所以第一次需要時才建立。
pub fn audio_context() -> Option<AudioContext> {
    AUDIO.with(|a| {
        let mut a = a.borrow_mut();
        if a.is_none() {
            *a = AudioContext::new().ok();
        }
        let ctx = a.clone()?;
        let _ = ctx.resume();
        Some(ctx)
    })
}

/// 換動作時的輕柔鈴聲：兩個正弦波疊加、慢慢淡出。
pub fn chime() {
    if let Some(ctx) = audio_context() {
        let t = ctx.current_time();
        for (freq, vol) in [(528.0_f32, 0.18_f32), (792.0, 0.06)] {
            let (Ok(osc), Ok(gain)) = (ctx.create_oscillator(), ctx.create_gain()) else {
                return;
            };
            osc.set_type(OscillatorType::Sine);
            osc.frequency().set_value(freq);
            let g = gain.gain();
            let _ = g.set_value_at_time(0.0, t);
            let _ = g.linear_ramp_to_value_at_time(vol, t + 0.04);
            let _ = g.exponential_ramp_to_value_at_time(0.0001, t + 2.8);
            let _ = osc.connect_with_audio_node(&gain);
            let _ = gain.connect_with_audio_node(&ctx.destination());
            let _ = osc.start();
            let _ = osc.stop_with_when(t + 3.0);
        }
    }
}

/// 念一句提示：有預錄語音檔（voice/<id>.mp3）就播檔案，沒有或播不出來就用內建語音念 [text]。
pub fn say(id: &str, text: &str) {
    stop_speaking();
    let seq = SAY_SEQ.get();
    let audio = HtmlAudioElement::new_with_src(&format!("voice/{id}.mp3")).ok();
    let Some(audio) = audio else {
        speak(text);
        return;
    };
    audio.set_volume(0.95);
    let Ok(promise) = audio.play() else {
        speak(text);
        return;
    };
    CLIP.with(|c| *c.borrow_mut() = Some(audio));
    let text = text.to_string();
    spawn_local(async move {
        // 檔案不存在（404）時 play() 會失敗；被下一句打斷也會失敗，但那時 SAY_SEQ 已經變了
        if JsFuture::from(promise).await.is_err() && SAY_SEQ.get() == seq {
            speak(&text);
        }
    });
}

/// 用瀏覽器內建語音合成念出提示（台灣華語、放慢語速）。
fn speak(text: &str) {
    let Some(win) = web_sys::window() else { return };
    let Ok(synth) = win.speech_synthesis() else {
        return;
    };
    let Ok(u) = SpeechSynthesisUtterance::new_with_text(text) else {
        return;
    };
    u.set_lang("zh-TW");
    u.set_rate(0.85);
    u.set_pitch(0.95);
    u.set_volume(0.9);
    if let Ok(voices) = Reflect::get(&synth, &"getVoices".into())
        .and_then(|f| f.dyn_into::<Function>().map_err(JsValue::from))
        .and_then(|f| f.call0(&synth))
    {
        let voices: js_sys::Array = voices.unchecked_into();
        let pick = voices.iter().find(|v| {
            Reflect::get(v, &"lang".into())
                .ok()
                .and_then(|l| l.as_string())
                .map(|l| l.replace('_', "-").eq_ignore_ascii_case("zh-TW"))
                .unwrap_or(false)
        });
        if let Some(v) = pick {
            let _ = Reflect::set(&u, &"voice".into(), &v);
        }
    }
    synth.cancel();
    synth.speak(&u);
}

pub fn stop_speaking() {
    SAY_SEQ.set(SAY_SEQ.get().wrapping_add(1));
    CLIP.with(|c| {
        if let Some(a) = c.borrow_mut().take() {
            let _ = a.pause();
        }
    });
    if let Some(Ok(s)) = web_sys::window().map(|w| w.speech_synthesis()) {
        s.cancel();
    }
}

/// 練習時讓螢幕保持亮著（Screen Wake Lock API，不支援的瀏覽器會略過）。
pub fn keep_awake() {
    let Some(win) = web_sys::window() else { return };
    let nav = win.navigator();
    let Ok(wl) = Reflect::get(&nav, &"wakeLock".into()) else {
        return;
    };
    if wl.is_undefined() || wl.is_null() {
        return;
    }
    let Ok(req) = Reflect::get(&wl, &"request".into()) else {
        return;
    };
    let Ok(req) = req.dyn_into::<Function>() else {
        return;
    };
    let Ok(p) = req.call1(&wl, &"screen".into()) else {
        return;
    };
    let Ok(p) = p.dyn_into::<Promise>() else {
        return;
    };
    spawn_local(async move {
        if let Ok(sentinel) = JsFuture::from(p).await {
            WAKE.with(|w| *w.borrow_mut() = Some(sentinel));
        }
    });
}

pub fn release_awake() {
    WAKE.with(|w| {
        if let Some(s) = w.borrow_mut().take() {
            if let Ok(f) = Reflect::get(&s, &"release".into())
                .and_then(|f| f.dyn_into::<Function>().map_err(JsValue::from))
            {
                let _ = f.call0(&s);
            }
        }
    });
}

pub fn load(key: &str) -> Option<String> {
    web_sys::window()?
        .local_storage()
        .ok()??
        .get_item(key)
        .ok()?
}

pub fn save(key: &str, value: &str) {
    if let Some(Ok(Some(s))) = web_sys::window().map(|w| w.local_storage()) {
        let _ = s.set_item(key, value);
    }
}

/// 今天往前推 `days_ago` 天的日期字串（本地時間，YYYY-MM-DD）。
pub fn date_key(days_ago: u32) -> String {
    let now = js_sys::Date::now() - f64::from(days_ago) * 86_400_000.0;
    let d = js_sys::Date::new(&JsValue::from_f64(now));
    format!(
        "{:04}-{:02}-{:02}",
        d.get_full_year(),
        d.get_month() + 1,
        d.get_date()
    )
}

/// 今天是星期幾（0 = 週日）。
pub fn weekday_today() -> u32 {
    js_sys::Date::new_0().get_day()
}

/// 匿名完成統計的收集網址（Google Apps Script，見 cornming.github.io 的 /stats/）。
const STATS_URL: &str = "https://script.google.com/macros/s/AKfycbzN1cM7oU-eeatBeISxnm3n8CbYiQej-aOfmIGljFUAAFmRtcP-9NruWs8_RMkhsSpkVA/exec";

/// 回報「完成一次」。只送作品、課程代號、當下的小時（本地時間 0 到 23）、這個瀏覽器第幾次完成的分級
/// （1、2-3、4+），不送任何能識別個人的資料。
/// 統計只是附加功能：任何一步出錯都靜靜放棄，絕不能影響使用者繼續使用。
pub fn report_completion(app: &str, course: &str, nth: &str) {
    let _ = try_report(app, course, nth);
}

/// 每一步都用 `?` 往外丟：沒有 window、沒有 fetch、fetch 同步丟出例外時只會得到 None。
/// （WASM 一旦 panic 整個頁面就停了，所以這裡不能有 unwrap，也不能用會丟例外的綁定。）
fn try_report(app: &str, course: &str, nth: &str) -> Option<()> {
    let window = web_sys::window()?;
    let fetch = Reflect::get(&window, &JsValue::from_str("fetch")).ok()?.dyn_into::<Function>().ok()?;
    let hour = js_sys::Date::new_0().get_hours();
    let body = crate::comeback::completion_body(app, course, hour, nth);
    let init = js_sys::Object::new();
    // Apps Script 不支援跨網域預檢，所以用 no-cors 的簡單請求；回應讀不到，也不需要。
    // keepalive：使用者回報後馬上關掉分頁，請求也能送完。
    for (key, value) in [
        ("method", JsValue::from_str("POST")),
        ("mode", JsValue::from_str("no-cors")),
        ("body", JsValue::from_str(&body)),
        ("keepalive", JsValue::TRUE),
    ] {
        Reflect::set(&init, &JsValue::from_str(key), &value).ok()?;
    }
    let promise = fetch.call2(&window, &JsValue::from_str(STATS_URL), &init).ok()?.dyn_into::<Promise>().ok()?;
    spawn_local(async move {
        // 網路不通、被擋、逾時都會到這裡，直接忽略
        let _ = JsFuture::from(promise).await;
    });
    Some(())
}

/// 這個瀏覽器的累計次數加一，回傳新的次數。儲存空間不能用時回傳 1，不影響其他功能。
pub fn bump_counter(key: &str) -> u32 {
    let n = load(key)
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(0)
        .saturating_add(1);
    save(key, &n.to_string());
    n
}

/// 明天的日期（本地時間），格式 YYYYMMDD。
pub fn tomorrow_ymd() -> String {
    let d = js_sys::Date::new(&JsValue::from_f64(js_sys::Date::now() + 86_400_000.0));
    format!("{:04}{:02}{:02}", d.get_full_year(), d.get_month() + 1, d.get_date())
}

/// 把文字存成檔案讓使用者下載（用 data: 網址，不需要額外的 Blob API）。
pub fn download_text(filename: &str, mime: &str, text: &str) {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else { return };
    let Ok(a) = doc.create_element("a") else { return };
    let enc: String = js_sys::encode_uri_component(text).into();
    let _ = a.set_attribute("href", &format!("data:{mime};charset=utf-8,{enc}"));
    let _ = a.set_attribute("download", filename);
    if let Ok(a) = a.dyn_into::<web_sys::HtmlElement>() {
        a.click();
    }
}
