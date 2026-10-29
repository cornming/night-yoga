use crate::data::{Breathing, DATA, Phase, Step, StepKind};
use crate::music::{self, Music};
use crate::web;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

const KEY_LOG: &str = "night-yoga:log";
const KEY_SETTINGS: &str = "night-yoga:settings";
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    voice: bool,
    chime: bool,
    /// 畫面亮度百分比（40–100）
    brightness: u32,
    music: Music,
    /// 音樂音量（0–100）
    music_vol: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            voice: true,
            chime: true,
            brightness: 80,
            music: Music::Crystal,
            music_vol: 55,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Screen {
    Home,
    Player,
    Done { secs: u32 },
}

/// 整個 App 共用的狀態，全部都是可複製的 signal。
#[derive(Clone, Copy)]
struct Session {
    screen: RwSignal<Screen>,
    title: RwSignal<String>,
    steps: RwSignal<Vec<Step>>,
    idx: RwSignal<usize>,
    remaining: RwSignal<u32>,
    paused: RwSignal<bool>,
    practiced: RwSignal<u32>,
    last_phase: RwSignal<Option<Phase>>,
    settings: RwSignal<Settings>,
    log: RwSignal<BTreeMap<String, u32>>,
    /// 首頁正在試聽背景音樂
    preview: RwSignal<bool>,
}

impl Session {
    fn new() -> Self {
        let settings = web::load(KEY_SETTINGS)
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let log = web::load(KEY_LOG)
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self {
            screen: RwSignal::new(Screen::Home),
            title: RwSignal::new(String::new()),
            steps: RwSignal::new(Vec::new()),
            idx: RwSignal::new(0),
            remaining: RwSignal::new(0),
            paused: RwSignal::new(false),
            practiced: RwSignal::new(0),
            last_phase: RwSignal::new(None),
            settings: RwSignal::new(settings),
            log: RwSignal::new(log),
            preview: RwSignal::new(false),
        }
    }

    fn update_settings(&self, f: impl FnOnce(&mut Settings)) {
        self.settings.update(f);
        let s = self.settings.get_untracked();
        web::save(KEY_SETTINGS, &serde_json::to_string(&s).unwrap_or_default());
    }

    /// 練習中或試聽中，讓正在播的音樂跟上目前的設定。
    fn apply_music(&self) {
        let st = self.settings.get_untracked();
        if self.screen.get_untracked() != Screen::Player && !self.preview.get_untracked() {
            return;
        }
        if st.music == Music::None {
            music::stop(1.5);
            self.preview.set(false);
        } else {
            music::play(st.music, st.music_vol);
        }
    }

    fn choose_music(&self, m: Music) {
        self.update_settings(|x| x.music = m);
        self.apply_music();
    }

    fn cycle_music(&self) {
        let cur = self.settings.get_untracked().music;
        let i = Music::ALL.iter().position(|m| *m == cur).unwrap_or(0);
        self.choose_music(Music::ALL[(i + 1) % Music::ALL.len()]);
    }

    fn set_music_volume(&self, v: u32) {
        self.update_settings(|x| x.music_vol = v.min(100));
        music::set_volume(v);
    }

    fn toggle_preview(&self) {
        if self.preview.get_untracked() {
            music::stop(1.0);
            self.preview.set(false);
        } else {
            let st = self.settings.get_untracked();
            if st.music != Music::None {
                music::play(st.music, st.music_vol);
                self.preview.set(true);
            }
        }
    }

    fn start(&self, title: String, steps: Vec<Step>) {
        if steps.is_empty() {
            return;
        }
        self.remaining.set(steps[0].seconds);
        self.title.set(title);
        self.steps.set(steps);
        self.idx.set(0);
        self.paused.set(false);
        self.practiced.set(0);
        self.screen.set(Screen::Player);
        web::keep_awake();
        self.preview.set(false);
        let st = self.settings.get_untracked();
        if st.music == Music::None {
            music::stop(1.0);
        } else {
            music::play(st.music, st.music_vol);
        }
        self.announce();
        scroll_top();
    }

    fn announce(&self) {
        self.last_phase.set(None);
        let s = self.settings.get_untracked();
        if s.chime {
            web::chime();
        }
        if !s.voice {
            return;
        }
        let Some(step) = self
            .steps
            .with_untracked(|v| v.get(self.idx.get_untracked()).cloned())
        else {
            return;
        };
        let text = match step.kind {
            StepKind::Pose { idx, side } => {
                let p = &DATA.poses[idx];
                match side {
                    Some(sd) => format!("{}，{}。{}", p.zh, sd.label(), p.cue),
                    None => format!("{}。{}", p.zh, p.cue),
                }
            }
            StepKind::Breath { idx } => format!("{}，跟著圓圈慢慢呼吸。", DATA.breathing[idx].name),
        };
        web::speak(&text);
    }

    fn goto(&self, i: usize) {
        let len = self.steps.with_untracked(|v| v.len());
        if i >= len {
            self.finish();
            return;
        }
        let secs = self.steps.with_untracked(|v| v[i].seconds);
        self.idx.set(i);
        self.remaining.set(secs);
        self.announce();
    }

    fn prev(&self) {
        let i = self.idx.get_untracked();
        let secs = self.steps.with_untracked(|v| v[i].seconds);
        // 剛開始幾秒內按「上一個」才回到上一步，否則重來目前這一步
        if secs - self.remaining.get_untracked() > 3 || i == 0 {
            self.goto(i);
        } else {
            self.goto(i - 1);
        }
    }

    fn tick(&self) {
        if self.screen.get_untracked() != Screen::Player || self.paused.get_untracked() {
            return;
        }
        self.practiced.update(|p| *p += 1);
        let rem = self.remaining.get_untracked();
        if rem <= 1 {
            self.goto(self.idx.get_untracked() + 1);
            return;
        }
        self.remaining.set(rem - 1);

        // 呼吸步驟：每換一個階段念一次口令
        let step = self
            .steps
            .with_untracked(|v| v.get(self.idx.get_untracked()).cloned());
        if let Some(Step {
            kind: StepKind::Breath { idx },
            seconds,
        }) = step
        {
            let info = DATA.breathing[idx].phase_at(seconds - (rem - 1));
            if self.last_phase.get_untracked() != Some(info.phase) {
                self.last_phase.set(Some(info.phase));
                if info.phase != Phase::Prepare && self.settings.get_untracked().voice {
                    web::speak(info.phase.label());
                }
            }
        }
    }

    fn record(&self) -> u32 {
        let secs = self.practiced.get_untracked();
        if secs >= 60 {
            let mins = (secs + 30) / 60;
            self.log
                .update(|l| *l.entry(web::date_key(0)).or_insert(0) += mins);
            web::save(
                KEY_LOG,
                &self
                    .log
                    .with_untracked(|l| serde_json::to_string(l).unwrap_or_default()),
            );
        }
        secs
    }

    fn cleanup(&self, fade: f64) {
        web::release_awake();
        music::stop(fade);
        web::stop_speaking();
    }

    fn finish(&self) {
        let secs = self.record();
        // 結束時音樂慢慢淡出，不要突然停掉
        self.cleanup(8.0);
        if self.settings.get_untracked().voice {
            web::speak("練習結束，晚安，好好睡一覺。");
        }
        self.screen.set(Screen::Done { secs });
        scroll_top();
    }

    fn exit(&self) {
        self.record();
        self.cleanup(1.5);
        self.screen.set(Screen::Home);
        scroll_top();
    }

    fn toggle_pause(&self) {
        let p = !self.paused.get_untracked();
        self.paused.set(p);
        if p {
            web::stop_speaking();
        }
    }
}

fn scroll_top() {
    if let Some(w) = web_sys::window() {
        w.scroll_to_with_x_and_y(0.0, 0.0);
    }
}

fn streak(log: &BTreeMap<String, u32>) -> u32 {
    let has = |d: u32| log.get(&web::date_key(d)).copied().unwrap_or(0) > 0;
    let mut d = if has(0) { 0 } else { 1 };
    let mut n = 0;
    while has(d) {
        n += 1;
        d += 1;
    }
    n
}

fn mmss(secs: u32) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

#[component]
pub fn App() -> impl IntoView {
    let s = Session::new();
    provide_context(s);
    set_interval(move || s.tick(), Duration::from_millis(1000));
    set_interval(music::tick, Duration::from_millis(400));

    view! {
        <div class="app" style:filter=move || format!("brightness({}%)", s.settings.get().brightness)>
            <div class="stars" aria-hidden="true"></div>
            <main class="wrap">
                {move || match s.screen.get() {
                    Screen::Home => view! { <Home/> }.into_any(),
                    Screen::Player => view! { <Player/> }.into_any(),
                    Screen::Done { secs } => view! { <Done secs/> }.into_any(),
                }}
            </main>
        </div>
    }
}

#[component]
fn Home() -> impl IntoView {
    let s = expect_context::<Session>();

    let routines = DATA
        .routines
        .iter()
        .map(|r| {
            let title = r.title.clone();
            let steps = r.steps.clone();
            let chips = r.pose_names().into_iter().map(|n| view! { <span class="chip">{n}</span> }).collect_view();
            view! {
                <article class="card routine">
                    <div class="card-head">
                        <h3>{r.title.clone()}</h3>
                        <span class="mins">{format!("約 {} 分鐘", r.minutes())}</span>
                    </div>
                    <p class="desc">{r.desc.clone()}</p>
                    <div class="chips">{chips}</div>
                    <button class="btn primary" on:click=move |_| s.start(title.clone(), steps.clone())>
                        "開始"
                    </button>
                </article>
            }
        })
        .collect_view();

    let breaths = DATA
        .breathing
        .iter()
        .enumerate()
        .map(|(idx, b)| {
            let title = b.name.clone();
            let total = b.total();
            view! {
                <article class="card breath-card">
                    <div class="card-head">
                        <h3>{b.name.clone()}</h3>
                        <span class="mins">{mmss(total)}</span>
                    </div>
                    <p class="pattern">{b.pattern()}</p>
                    <p class="desc">{b.desc.clone()}</p>
                    <button
                        class="btn ghost"
                        on:click=move |_| s.start(title.clone(), vec![Step { kind: StepKind::Breath { idx }, seconds: total }])
                    >
                        "開始呼吸"
                    </button>
                </article>
            }
        })
        .collect_view();

    view! {
        <header class="hero">
            <div class="moon" aria-hidden="true"></div>
            <h1>"晚安瑜伽"</h1>
            <p class="sub">"睡前 5 到 15 分鐘，伸展、呼吸，慢慢把今天放下。"</p>
        </header>

        <aside class="safety" role="note">
            <strong>"開始前"</strong>
            "這裡都是溫和的伸展，不安排倒立和強烈後彎。任何動作感到疼痛就停下來；有傷病或正在懷孕，請先詢問醫師。"
        </aside>

        <section>
            <h2>"睡前課程"</h2>
            <div class="grid">{routines}</div>
        </section>

        <section>
            <h2>"只想呼吸"</h2>
            <div class="grid three">{breaths}</div>
        </section>

        <Stats/>
        <MusicPanel/>
        <SettingsPanel/>

        <footer class="foot">
            <span>{format!("v{VERSION}")}</span>
            <span>"練習紀錄只存在這台裝置的瀏覽器裡"</span>
        </footer>
    }
}

#[component]
fn Stats() -> impl IntoView {
    let s = expect_context::<Session>();
    let today_wd = web::weekday_today() as i32;

    let cells = move || {
        let log = s.log.get();
        (0..35)
            .map(|i| {
                let ago = (34 - i) - (6 - today_wd);
                if ago < 0 {
                    return view! { <span class="cell future"></span> }.into_any();
                }
                let key = web::date_key(ago as u32);
                let m = log.get(&key).copied().unwrap_or(0);
                let lvl = match m {
                    0 => "l0",
                    1..=5 => "l1",
                    6..=10 => "l2",
                    _ => "l3",
                };
                let tip = if m > 0 {
                    format!("{key}：{m} 分鐘")
                } else {
                    key.clone()
                };
                view! { <span class=format!("cell {lvl}") class:today=ago == 0 title=tip></span> }
                    .into_any()
            })
            .collect_view()
    };

    view! {
        <section class="card stats">
            <h2>"練習紀錄"</h2>
            <div class="nums">
                <div>
                    <b>{move || s.log.with(streak)}</b>
                    <span>"連續天數"</span>
                </div>
                <div>
                    <b>{move || s.log.with(|l| l.values().filter(|m| **m > 0).count())}</b>
                    <span>"練習天數"</span>
                </div>
                <div>
                    <b>{move || s.log.with(|l| l.values().sum::<u32>())}</b>
                    <span>"累計分鐘"</span>
                </div>
            </div>
            <div class="cal-head">
                {["日", "一", "二", "三", "四", "五", "六"].map(|d| view! { <span>{d}</span> }).collect_view()}
            </div>
            <div class="cal">{cells}</div>
            <p class="hint">"每次練習滿 1 分鐘會記錄下來。"</p>
        </section>
    }
}

#[component]
fn Toggle(label: &'static str, on: Signal<bool>, #[prop(into)] set: Callback<()>) -> impl IntoView {
    view! {
        <button
            class="toggle"
            class:on=move || on.get()
            role="switch"
            aria-checked=move || on.get().to_string()
            on:click=move |_| set.run(())
        >
            <span class="knob" aria-hidden="true"></span>
            {label}
        </button>
    }
}

#[component]
fn SettingsPanel() -> impl IntoView {
    let s = expect_context::<Session>();
    let voice = Signal::derive(move || s.settings.get().voice);
    let chime = Signal::derive(move || s.settings.get().chime);

    view! {
        <section class="card settings">
            <h2>"聲音與畫面"</h2>
            <div class="toggles">
                <Toggle label="語音提示" on=voice set=move |_| s.update_settings(|x| x.voice = !x.voice)/>
                <Toggle label="換式鈴聲" on=chime set=move |_| s.update_settings(|x| x.chime = !x.chime)/>
            </div>
            <BrightnessSlider/>
        </section>
    }
}

#[component]
fn MusicPanel() -> impl IntoView {
    let s = expect_context::<Session>();
    let chips = Music::ALL
        .into_iter()
        .map(|m| {
            view! {
                <button
                    class="mchip"
                    class:on=move || s.settings.get().music == m
                    aria-pressed=move || (s.settings.get().music == m).to_string()
                    on:click=move |_| s.choose_music(m)
                >
                    {m.label()}
                </button>
            }
        })
        .collect_view();

    view! {
        <section class="card music">
            <div class="card-head">
                <h2>"背景音樂"</h2>
                <button
                    class="btn ghost small"
                    disabled=move || s.settings.get().music == Music::None
                    on:click=move |_| s.toggle_preview()
                >
                    {move || if s.preview.get() { "■ 停止試聽" } else { "▶ 試聽" }}
                </button>
            </div>
            <div class="mchips">{chips}</div>
            <p class="desc">{move || s.settings.get().music.desc()}</p>
            <label class="slider">
                <span>"音樂音量"</span>
                <input
                    type="range"
                    min="0"
                    max="100"
                    step="5"
                    prop:value=move || s.settings.get().music_vol.to_string()
                    on:input=move |ev| {
                        if let Ok(v) = event_target_value(&ev).parse::<u32>() {
                            s.set_music_volume(v);
                        }
                    }
                />
                <span class="val">{move || format!("{}%", s.settings.get().music_vol)}</span>
            </label>
            <p class="hint">"音樂都是瀏覽器即時合成的，不需要下載，離線也能播放。"</p>
        </section>
    }
}

#[component]
fn BrightnessSlider() -> impl IntoView {
    let s = expect_context::<Session>();
    view! {
        <label class="slider">
            <span>"畫面亮度"</span>
            <input
                type="range"
                min="40"
                max="100"
                step="5"
                prop:value=move || s.settings.get().brightness.to_string()
                on:input=move |ev| {
                    if let Ok(v) = event_target_value(&ev).parse::<u32>() {
                        s.update_settings(|x| x.brightness = v.clamp(40, 100));
                    }
                }
            />
            <span class="val">{move || format!("{}%", s.settings.get().brightness)}</span>
        </label>
    }
}

#[component]
fn Player() -> impl IntoView {
    let s = expect_context::<Session>();
    let current = Memo::new(move |_| s.steps.with(|v| v.get(s.idx.get()).cloned()));
    let count = move || s.steps.with(|v| v.len());
    let next_title = move || s.steps.with(|v| v.get(s.idx.get() + 1).map(|x| x.title()));

    let progress = move || {
        let i = s.idx.get();
        let rem = s.remaining.get();
        s.steps.with(|v| {
            let total: u32 = v.iter().map(|x| x.seconds).sum();
            let before: u32 = v.iter().take(i).map(|x| x.seconds).sum();
            let cur = v.get(i).map(|x| x.seconds.saturating_sub(rem)).unwrap_or(0);
            if total == 0 {
                0.0
            } else {
                f64::from(before + cur) / f64::from(total) * 100.0
            }
        })
    };

    view! {
        <div class="player">
            <div class="topbar">
                <div>
                    <div class="ptitle">{move || s.title.get()}</div>
                    <div class="pstep">{move || format!("第 {} / {} 步", s.idx.get() + 1, count())}</div>
                </div>
                <div class="top-actions">
                    <button
                        class="music-btn"
                        aria-label="切換背景音樂"
                        on:click=move |_| s.cycle_music()
                    >
                        {move || format!("♪ {}", s.settings.get().music.label())}
                    </button>
                    <button class="icon-btn" aria-label="結束練習" on:click=move |_| s.exit()>"✕"</button>
                </div>
            </div>
            <div class="bar"><span style:width=move || format!("{:.1}%", progress())></span></div>

            {move || match current.get() {
                Some(Step { kind: StepKind::Pose { idx, side }, seconds }) => {
                    view! { <PoseView idx side=side.map(|x| x.label()) seconds/> }.into_any()
                }
                Some(Step { kind: StepKind::Breath { idx }, seconds }) => {
                    view! { <BreathView idx seconds/> }.into_any()
                }
                None => ().into_any(),
            }}

            <div class="next">
                {move || match next_title() {
                    Some(t) => format!("下一個：{t}"),
                    None => "最後一個動作".to_string(),
                }}
            </div>

            <div class="controls">
                <button class="icon-btn big" aria-label="上一個" on:click=move |_| s.prev()>"⏮"</button>
                <button
                    class="icon-btn huge"
                    aria-label=move || if s.paused.get() { "繼續" } else { "暫停" }
                    on:click=move |_| s.toggle_pause()
                >
                    {move || if s.paused.get() { "▶" } else { "❚❚" }}
                </button>
                <button
                    class="icon-btn big"
                    aria-label="下一個"
                    on:click=move |_| s.goto(s.idx.get_untracked() + 1)
                >
                    "⏭"
                </button>
            </div>
            <Show when=move || s.paused.get()>
                <p class="paused">"已暫停"</p>
            </Show>
        </div>
    }
}

#[component]
fn Figure(idx: usize) -> impl IntoView {
    // SVG 由 figure 模組依骨架算好，直接放進頁面（內容來自程式內建資料）
    view! { <div class="figure-wrap" inner_html=DATA.poses[idx].svg.clone()></div> }
}

#[component]
fn PoseView(idx: usize, side: Option<&'static str>, seconds: u32) -> impl IntoView {
    let s = expect_context::<Session>();
    let p = &DATA.poses[idx];
    const C: f64 = 2.0 * std::f64::consts::PI * 54.0;
    let offset = move || {
        let r = f64::from(s.remaining.get()) / f64::from(seconds.max(1));
        format!("{:.2}", C * (1.0 - r))
    };
    let steps = p
        .steps
        .iter()
        .map(|t| view! { <li>{t.clone()}</li> })
        .collect_view();

    view! {
        <section class="pose">
            <Figure idx/>
            <div class="names">
                <h2>{p.zh.clone()}</h2>
                {side.map(|sd| view! { <span class="side">{sd}</span> })}
                <p class="latin">{format!("{} · {}", p.en, p.sa)}</p>
            </div>
            <div class="ring">
                <svg viewBox="0 0 120 120" aria-hidden="true">
                    <circle class="track" cx="60" cy="60" r="54"></circle>
                    <circle
                        class="arc"
                        cx="60"
                        cy="60"
                        r="54"
                        stroke-dasharray=format!("{C:.2}")
                        stroke-dashoffset=offset
                    ></circle>
                </svg>
                <span class="count">{move || mmss(s.remaining.get())}</span>
            </div>
            <ol class="steps">{steps}</ol>
            {p.caution.clone().map(|c| view! { <p class="caution">{c}</p> })}
        </section>
    }
}

#[component]
fn BreathView(idx: usize, seconds: u32) -> impl IntoView {
    let s = expect_context::<Session>();
    let b: &'static Breathing = &DATA.breathing[idx];
    let info = Memo::new(move |_| b.phase_at(seconds.saturating_sub(s.remaining.get())));

    view! {
        <section class="breath">
            <div class="orb-wrap">
                <div
                    class="orb"
                    class:paused=move || s.paused.get()
                    style:transform=move || format!("scale({})", info.get().phase.scale())
                    style:transition-duration=move || format!("{}s", info.get().len)
                ></div>
                <div class="orb-text">
                    <span class="phase">{move || info.get().phase.label()}</span>
                    <span class="left">{move || info.get().left}</span>
                </div>
            </div>
            <h2>{b.name.clone()}</h2>
            <p class="pattern">{b.pattern()}</p>
            <p class="cycle">
                {move || {
                    let i = info.get();
                    if i.cycle == 0 { "坐好或躺好，先自然呼吸".to_string() } else { format!("第 {} / {} 輪", i.cycle, b.cycles) }
                }}
            </p>
        </section>
    }
}

#[component]
fn Done(secs: u32) -> impl IntoView {
    let s = expect_context::<Session>();
    let mins = (secs + 30) / 60;
    view! {
        <section class="done">
            <div class="moon big" aria-hidden="true"></div>
            <h1>"晚安"</h1>
            <p class="sub">
                {if secs >= 60 { format!("今天練習了 {mins} 分鐘。") } else { "今天也辛苦了。".to_string() }}
            </p>
            <p class="streak">{move || format!("連續 {} 天", s.log.with(streak))}</p>
            <p class="desc">"放下手機，關燈，讓呼吸維持剛才的節奏。"</p>
            <button class="btn primary" on:click=move |_| s.screen.set(Screen::Home)>"回到首頁"</button>
        </section>
    }
}
