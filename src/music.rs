//! 背景音樂：全部用 Web Audio 即時合成，不需要音檔，離線可用、也沒有版權問題。
//! 每種音色有自己的「生成規則」，由 App 每 0.4 秒呼叫 `tick()` 預先排好接下來一秒多的音符。

use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use web_sys::{
    AudioBuffer, AudioContext, AudioNode, AudioScheduledSourceNode, BiquadFilterNode,
    BiquadFilterType, ConvolverNode, GainNode, OscillatorNode, OscillatorType,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Music {
    #[default]
    None,
    Crystal,
    Bowls,
    Pad,
    Rain,
    Ocean,
}

impl Music {
    pub const ALL: [Music; 6] = [
        Music::None,
        Music::Crystal,
        Music::Bowls,
        Music::Pad,
        Music::Rain,
        Music::Ocean,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Music::None => "不播放",
            Music::Crystal => "水晶鋼琴",
            Music::Bowls => "頌缽",
            Music::Pad => "溫暖長音",
            Music::Rain => "雨聲",
            Music::Ocean => "海浪",
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            Music::None => "安靜練習，只保留語音和鈴聲。",
            Music::Crystal => "清透的鐘琴音色，五聲音階緩緩漂浮。",
            Music::Bowls => "低沉悠長的缽音，餘韻帶著輕微的嗡鳴。",
            Music::Pad => "柔和的和弦長音，慢慢流動，最不搶注意力。",
            Music::Rain => "遠處綿密的雨，蓋過環境雜音。",
            Music::Ocean => "一波一波的浪，可以跟著浪的節奏呼吸。",
        }
    }

    /// 各音色的音量平衡
    fn level(self) -> f32 {
        match self {
            Music::None => 0.0,
            Music::Crystal => 0.55,
            Music::Bowls => 0.5,
            Music::Pad => 0.7,
            Music::Rain => 0.85,
            Music::Ocean => 0.8,
        }
    }

    /// 送進殘響的比例
    fn reverb(self) -> f32 {
        match self {
            Music::Crystal => 0.75,
            Music::Bowls => 0.55,
            Music::Pad => 0.4,
            _ => 0.0,
        }
    }
}

struct Track {
    kind: Music,
    bus: GainNode,
    /// 持續發聲的來源（噪音、長音），停止時要一起關掉
    sources: Vec<AudioScheduledSourceNode>,
    /// 溫暖長音的三個聲部（每個聲部兩個振盪器）
    voices: Vec<Vec<OscillatorNode>>,
    /// 海浪用的音量與濾波器
    swell: Option<(GainNode, BiquadFilterNode)>,
    next: f64,
    step: u32,
    note: usize,
}

struct Engine {
    ctx: AudioContext,
    master: GainNode,
    reverb: ConvolverNode,
    track: Option<Track>,
}

thread_local! {
    static ENGINE: RefCell<Option<Engine>> = const { RefCell::new(None) };
}

fn rand() -> f64 {
    js_sys::Math::random()
}
fn rand_range(a: f64, b: f64) -> f64 {
    a + (b - a) * rand()
}

fn with_engine(f: impl FnOnce(&mut Engine)) {
    ENGINE.with(|e| {
        let mut e = e.borrow_mut();
        if e.is_none() {
            let Some(ctx) = crate::web::audio_context() else {
                return;
            };
            if let Some(engine) = Engine::new(ctx) {
                *e = Some(engine);
            }
        }
        if let Some(engine) = e.as_mut() {
            let _ = engine.ctx.resume();
            f(engine);
        }
    });
}

/// 產生一段噪音；brown = true 時是低沉的棕噪音，否則是白噪音。
fn noise_buffer(ctx: &AudioContext, secs: f32, brown: bool) -> Option<AudioBuffer> {
    let rate = ctx.sample_rate();
    let len = (rate * secs) as u32;
    let buf = ctx.create_buffer(2, len, rate).ok()?;
    for ch in 0..2 {
        let mut data = vec![0.0_f32; len as usize];
        let mut last = 0.0_f32;
        for v in data.iter_mut() {
            let white = rand() as f32 * 2.0 - 1.0;
            if brown {
                last = (last + 0.02 * white) / 1.02;
                *v = last * 3.2;
            } else {
                *v = white * 0.5;
            }
        }
        // 頭尾淡入淡出，避免循環接縫處出現爆音
        let fade = (rate * 0.05) as usize;
        let n = data.len();
        for i in 0..fade {
            let k = i as f32 / fade as f32;
            data[i] *= k;
            data[n - 1 - i] *= k;
        }
        let _ = buf.copy_to_channel(&data, ch);
    }
    Some(buf)
}

/// 合成殘響用的脈衝響應：衰減的雙聲道噪音，約 4 秒尾音。
fn impulse(ctx: &AudioContext) -> Option<AudioBuffer> {
    let rate = ctx.sample_rate();
    let secs = 4.2_f32;
    let len = (rate * secs) as u32;
    let buf = ctx.create_buffer(2, len, rate).ok()?;
    for ch in 0..2 {
        let data: Vec<f32> = (0..len)
            .map(|i| {
                let t = i as f32 / rate;
                let env = (-t * 1.6).exp() * (1.0 - t / secs);
                (rand() as f32 * 2.0 - 1.0) * env
            })
            .collect();
        let _ = buf.copy_to_channel(&data, ch);
    }
    Some(buf)
}

fn connect(a: &AudioNode, b: &AudioNode) {
    let _ = a.connect_with_audio_node(b);
}

impl Engine {
    fn new(ctx: AudioContext) -> Option<Self> {
        let master = ctx.create_gain().ok()?;
        master.gain().set_value(0.6);
        connect(&master, &ctx.destination());
        let reverb = ctx.create_convolver().ok()?;
        reverb.set_buffer(impulse(&ctx).as_ref());
        let wet = ctx.create_gain().ok()?;
        wet.gain().set_value(0.6);
        connect(&reverb, &wet);
        connect(&wet, &master);
        Some(Self {
            ctx,
            master,
            reverb,
            track: None,
        })
    }

    fn stop_track(&mut self, fade: f64) {
        if let Some(tr) = self.track.take() {
            let t = self.ctx.current_time();
            let g = tr.bus.gain();
            let _ = g.cancel_scheduled_values(t);
            let _ = g.set_value_at_time(g.value(), t);
            let _ = g.linear_ramp_to_value_at_time(0.0, t + fade);
            for s in tr.sources {
                let _ = s.stop_with_when(t + fade + 0.1);
            }
            for v in tr.voices.into_iter().flatten() {
                let _ = v.stop_with_when(t + fade + 0.1);
            }
        }
    }

    fn start_track(&mut self, kind: Music) {
        let ctx = &self.ctx;
        let Ok(bus) = ctx.create_gain() else { return };
        let t = ctx.current_time();
        let _ = bus.gain().set_value_at_time(0.0, t);
        let _ = bus
            .gain()
            .linear_ramp_to_value_at_time(kind.level(), t + 2.5);
        connect(&bus, &self.master);
        if kind.reverb() > 0.0 {
            if let Ok(send) = ctx.create_gain() {
                send.gain().set_value(kind.reverb());
                connect(&bus, &send);
                connect(&send, &self.reverb);
            }
        }
        let mut tr = Track {
            kind,
            bus,
            sources: Vec::new(),
            voices: Vec::new(),
            swell: None,
            next: t + 0.3,
            step: 0,
            note: 4,
        };
        match kind {
            Music::Rain => start_rain(ctx, &mut tr),
            Music::Ocean => start_ocean(ctx, &mut tr),
            Music::Pad => start_pad(ctx, &mut tr),
            _ => {}
        }
        self.track = Some(tr);
    }

    fn tick(&mut self) {
        let now = self.ctx.current_time();
        let ctx = self.ctx.clone();
        let Some(tr) = self.track.as_mut() else {
            return;
        };
        // 分頁在背景太久會累積延遲，直接跳到現在
        if tr.next < now - 1.0 {
            tr.next = now + 0.1;
        }
        while tr.next < now + 1.2 {
            let t = tr.next;
            match tr.kind {
                Music::Crystal => crystal_step(&ctx, tr, t),
                Music::Bowls => bowl_step(&ctx, tr, t),
                Music::Pad => pad_step(tr, t),
                Music::Ocean => wave_step(tr, t),
                Music::Rain | Music::None => tr.next = t + 3600.0,
            }
            tr.step += 1;
        }
    }
}

// ── 水晶鋼琴 ─────────────────────────────

/// D 大調五聲音階（D E F# A B）跨兩個多八度
const PENTA: [f32; 11] = [
    293.66, 329.63, 369.99, 440.00, 493.88, 587.33, 659.25, 739.99, 880.00, 987.77, 1174.66,
];

/// 鐘琴般的音色：基音加幾個較快消失的泛音。
fn bell(ctx: &AudioContext, out: &AudioNode, t: f64, freq: f32, vel: f32, pan: f32, decay: f64) {
    let Ok(p) = ctx.create_stereo_panner() else {
        return;
    };
    p.pan().set_value(pan);
    connect(&p, out);
    for (ratio, amp, dk) in [
        (1.0_f32, 1.0_f32, 1.0),
        (2.0, 0.32, 0.5),
        (3.0, 0.1, 0.28),
        (4.07, 0.07, 0.16),
    ] {
        let (Ok(o), Ok(g)) = (ctx.create_oscillator(), ctx.create_gain()) else {
            return;
        };
        o.set_type(OscillatorType::Sine);
        o.frequency().set_value(freq * ratio);
        let end = t + decay * dk;
        let gp = g.gain();
        let _ = gp.set_value_at_time(0.0, t);
        let _ = gp.linear_ramp_to_value_at_time(vel * amp * 0.22, t + 0.006);
        let _ = gp.exponential_ramp_to_value_at_time(0.0001, end);
        connect(&o, &g);
        connect(&g, &p);
        let _ = o.start_with_when(t);
        let _ = o.stop_with_when(end + 0.05);
    }
}

fn crystal_step(ctx: &AudioContext, tr: &mut Track, t: f64) {
    // 音符在音階上小步漫步，偶爾跳兩格
    let jump = [-2_i32, -1, -1, 1, 1, 2][(rand() * 6.0) as usize % 6];
    tr.note = (tr.note as i32 + jump).clamp(0, PENTA.len() as i32 - 1) as usize;
    let pan = rand_range(-0.6, 0.6) as f32;
    let vel = rand_range(0.55, 1.0) as f32;
    bell(ctx, &tr.bus, t, PENTA[tr.note], vel, pan, 3.6);
    if rand() < 0.22 && tr.note + 2 < PENTA.len() {
        bell(
            ctx,
            &tr.bus,
            t + 0.02,
            PENTA[tr.note + 2],
            vel * 0.6,
            -pan,
            3.0,
        );
    }
    // 每八個音墊一個低音
    if tr.step % 8 == 0 {
        let low = if (tr.step / 8) % 2 == 0 {
            146.83
        } else {
            110.0
        };
        bell(ctx, &tr.bus, t, low, 0.7, 0.0, 6.0);
    }
    let rest = if tr.step % 9 == 8 { 2.5 } else { 0.0 };
    tr.next = t + rand_range(0.9, 2.4) + rest;
}

// ── 頌缽 ─────────────────────────────────

const BOWLS: [f32; 5] = [196.00, 220.00, 261.63, 293.66, 329.63];

/// 頌缽：非整數倍的泛音，每個泛音由兩個微微失諧的振盪器組成，產生「嗡嗡」的拍頻。
fn strike(ctx: &AudioContext, out: &AudioNode, t: f64, base: f32, vel: f32) {
    let Ok(p) = ctx.create_stereo_panner() else {
        return;
    };
    p.pan().set_value(rand_range(-0.5, 0.5) as f32);
    connect(&p, out);
    let partials = [
        (1.0_f32, 1.0_f32, 16.0),
        (2.71, 0.45, 11.0),
        (5.33, 0.2, 6.0),
        (8.75, 0.09, 3.5),
    ];
    for (ratio, amp, decay) in partials {
        let beat = rand_range(0.5, 1.4) as f32;
        for detune in [-beat / 2.0, beat / 2.0] {
            let (Ok(o), Ok(g)) = (ctx.create_oscillator(), ctx.create_gain()) else {
                return;
            };
            o.set_type(OscillatorType::Sine);
            o.frequency().set_value(base * ratio + detune);
            let end = t + decay;
            let gp = g.gain();
            let _ = gp.set_value_at_time(0.0, t);
            let _ = gp.linear_ramp_to_value_at_time(vel * amp * 0.12, t + 0.012);
            let _ = gp.exponential_ramp_to_value_at_time(0.0001, end);
            connect(&o, &g);
            connect(&g, &p);
            let _ = o.start_with_when(t);
            let _ = o.stop_with_when(end + 0.05);
        }
    }
}

fn bowl_step(ctx: &AudioContext, tr: &mut Track, t: f64) {
    let i = (rand() * BOWLS.len() as f64) as usize % BOWLS.len();
    strike(ctx, &tr.bus, t, BOWLS[i], rand_range(0.7, 1.0) as f32);
    if rand() < 0.3 {
        let j = (i + 2) % BOWLS.len();
        strike(ctx, &tr.bus, t + rand_range(2.0, 3.5), BOWLS[j], 0.5);
    }
    tr.next = t + rand_range(8.0, 13.0);
}

// ── 溫暖長音 ─────────────────────────────

const CHORDS: [[f32; 3]; 4] = [
    [146.83, 220.00, 369.99], // D
    [123.47, 220.00, 293.66], // Bm7
    [98.00, 246.94, 369.99],  // Gmaj7
    [110.00, 246.94, 329.63], // A6sus
];

fn start_pad(ctx: &AudioContext, tr: &mut Track) {
    let Ok(filter) = ctx.create_biquad_filter() else {
        return;
    };
    filter.set_type(BiquadFilterType::Lowpass);
    filter.frequency().set_value(850.0);
    filter.q().set_value(0.4);
    connect(&filter, &tr.bus);
    // 濾波器的截止頻率慢慢起伏，讓長音有呼吸感
    if let (Ok(lfo), Ok(depth)) = (ctx.create_oscillator(), ctx.create_gain()) {
        lfo.frequency().set_value(0.05);
        depth.gain().set_value(350.0);
        connect(&lfo, &depth);
        let _ = depth.connect_with_audio_param(&filter.frequency());
        let _ = lfo.start();
        tr.sources.push(lfo.into());
    }
    for f in CHORDS[0] {
        let mut pair = Vec::new();
        for cents in [-7.0_f32, 7.0] {
            let (Ok(o), Ok(g)) = (ctx.create_oscillator(), ctx.create_gain()) else {
                return;
            };
            o.set_type(OscillatorType::Triangle);
            o.frequency().set_value(f);
            o.detune().set_value(cents);
            g.gain().set_value(0.09);
            connect(&o, &g);
            connect(&g, &filter);
            let _ = o.start();
            pair.push(o);
        }
        tr.voices.push(pair);
    }
    tr.next = ctx.current_time() + 14.0;
}

fn pad_step(tr: &mut Track, t: f64) {
    let chord = CHORDS[(tr.step as usize + 1) % CHORDS.len()];
    for (voice, f) in tr.voices.iter().zip(chord) {
        for o in voice {
            let _ = o.frequency().set_target_at_time(f, t, 2.2);
        }
    }
    tr.next = t + 14.0;
}

// ── 雨聲、海浪 ───────────────────────────

fn loop_source(
    ctx: &AudioContext,
    buf: &AudioBuffer,
    out: &AudioNode,
) -> Option<AudioScheduledSourceNode> {
    let src = ctx.create_buffer_source().ok()?;
    src.set_buffer(Some(buf));
    src.set_loop(true);
    connect(&src, out);
    let node: AudioScheduledSourceNode = src.into();
    let _ = node.start();
    Some(node)
}

fn start_rain(ctx: &AudioContext, tr: &mut Track) {
    // 低沉的雨幕
    if let (Some(buf), Ok(f), Ok(g)) = (
        noise_buffer(ctx, 6.0, true),
        ctx.create_biquad_filter(),
        ctx.create_gain(),
    ) {
        f.set_type(BiquadFilterType::Lowpass);
        f.frequency().set_value(900.0);
        g.gain().set_value(0.45);
        connect(&f, &g);
        connect(&g, &tr.bus);
        if let Some(s) = loop_source(ctx, &buf, &f) {
            tr.sources.push(s);
        }
    }
    // 細碎的雨滴聲
    if let (Some(buf), Ok(f), Ok(g)) = (
        noise_buffer(ctx, 5.0, false),
        ctx.create_biquad_filter(),
        ctx.create_gain(),
    ) {
        f.set_type(BiquadFilterType::Bandpass);
        f.frequency().set_value(4200.0);
        f.q().set_value(0.7);
        g.gain().set_value(0.06);
        connect(&f, &g);
        connect(&g, &tr.bus);
        if let Some(s) = loop_source(ctx, &buf, &f) {
            tr.sources.push(s);
        }
    }
}

fn start_ocean(ctx: &AudioContext, tr: &mut Track) {
    let (Some(buf), Ok(f), Ok(g)) = (
        noise_buffer(ctx, 6.0, false),
        ctx.create_biquad_filter(),
        ctx.create_gain(),
    ) else {
        return;
    };
    f.set_type(BiquadFilterType::Lowpass);
    f.frequency().set_value(400.0);
    g.gain().set_value(0.1);
    connect(&f, &g);
    connect(&g, &tr.bus);
    if let Some(s) = loop_source(ctx, &buf, &f) {
        tr.sources.push(s);
    }
    tr.swell = Some((g, f));
}

fn wave_step(tr: &mut Track, t: f64) {
    let Some((g, f)) = &tr.swell else {
        tr.next = t + 3600.0;
        return;
    };
    let rise = rand_range(3.0, 4.5);
    let fall = rand_range(4.5, 6.5);
    let peak = rand_range(0.55, 0.95) as f32;
    let gp = g.gain();
    let fp = f.frequency();
    let _ = gp.set_target_at_time(peak, t, rise / 3.0);
    let _ = gp.set_target_at_time(0.08, t + rise, fall / 3.0);
    let _ = fp.set_target_at_time(rand_range(900.0, 1400.0) as f32, t, rise / 3.0);
    let _ = fp.set_target_at_time(380.0, t + rise, fall / 3.0);
    tr.next = t + rise + fall + rand_range(0.0, 1.5);
}

// ── 對外介面 ─────────────────────────────

/// 播放指定的音樂；已經在播同一種就只調整音量，不重新開始。
pub fn play(kind: Music, volume: u32) {
    with_engine(|e| {
        set_master(e, volume);
        if e.track.as_ref().map(|t| t.kind) == Some(kind) {
            return;
        }
        e.stop_track(1.5);
        if kind != Music::None {
            e.start_track(kind);
            e.tick();
        }
    });
}

pub fn stop(fade: f64) {
    ENGINE.with(|e| {
        if let Some(e) = e.borrow_mut().as_mut() {
            e.stop_track(fade);
        }
    });
}

fn set_master(e: &Engine, volume: u32) {
    let t = e.ctx.current_time();
    let v = (volume.min(100) as f32 / 100.0).powf(1.6) * 0.9;
    let _ = e.master.gain().set_target_at_time(v, t, 0.15);
}

pub fn set_volume(volume: u32) {
    ENGINE.with(|e| {
        if let Some(e) = e.borrow().as_ref() {
            set_master(e, volume);
        }
    });
}

pub fn tick() {
    ENGINE.with(|e| {
        if let Some(e) = e.borrow_mut().as_mut() {
            e.tick();
        }
    });
}
