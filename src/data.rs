//! 課程、體式與呼吸程式的資料模型。
//! 內容放在 data/*.json，編譯時嵌入；要新增課程只需改 JSON。

use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Debug, Clone, Deserialize)]
pub struct Pose {
    pub id: String,
    pub zh: String,
    pub en: String,
    pub sa: String,
    pub cue: String,
    pub steps: Vec<String>,
    pub caution: Option<String>,
    /// 示範圖：一個影格是靜態圖，兩個影格會來回播放
    pub figure: Vec<crate::figure::Skeleton>,
    /// 兩個影格來回一次的秒數
    #[serde(default = "default_period")]
    pub period: f32,
    /// 由 figure 算出的 SVG
    #[serde(skip)]
    pub svg: String,
}

fn default_period() -> f32 {
    8.0
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Ratio {
    pub inhale: u32,
    pub retain: u32,
    pub exhale: u32,
    pub sustain: u32,
}

/// 呼吸程式，格式參考 nafas 的 JSON 設定（pre + cycles + ratio）。
#[derive(Debug, Clone, Deserialize)]
pub struct Breathing {
    pub id: String,
    pub name: String,
    pub desc: String,
    /// 開始前的準備秒數
    pub pre: u32,
    pub cycles: u32,
    pub ratio: Ratio,
}

impl Breathing {
    pub fn cycle_len(&self) -> u32 {
        let r = self.ratio;
        r.inhale + r.retain + r.exhale + r.sustain
    }
    pub fn total(&self) -> u32 {
        self.pre + self.cycles * self.cycle_len()
    }
    pub fn pattern(&self) -> String {
        let r = self.ratio;
        let mut parts = vec![format!("吸 {}", r.inhale)];
        if r.retain > 0 {
            parts.push(format!("停 {}", r.retain));
        }
        parts.push(format!("吐 {}", r.exhale));
        if r.sustain > 0 {
            parts.push(format!("停 {}", r.sustain));
        }
        parts.join(" · ")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Prepare,
    Inhale,
    Retain,
    Exhale,
    Sustain,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Prepare => "準備",
            Phase::Inhale => "吸氣",
            Phase::Retain => "停住",
            Phase::Exhale => "吐氣",
            Phase::Sustain => "停住",
        }
    }
    /// 呼吸圓圈的縮放比例：吸滿時大、吐完時小。
    pub fn scale(self) -> f32 {
        match self {
            Phase::Prepare => 0.55,
            Phase::Inhale | Phase::Retain => 1.0,
            Phase::Exhale | Phase::Sustain => 0.55,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseInfo {
    pub phase: Phase,
    /// 這個階段的總秒數
    pub len: u32,
    /// 這個階段剩下幾秒
    pub left: u32,
    /// 第幾輪（1 起算，準備階段為 0）
    pub cycle: u32,
}

impl Breathing {
    /// 依已經過的秒數算出目前的呼吸階段。
    pub fn phase_at(&self, elapsed: u32) -> PhaseInfo {
        if elapsed < self.pre {
            return PhaseInfo {
                phase: Phase::Prepare,
                len: self.pre,
                left: self.pre - elapsed,
                cycle: 0,
            };
        }
        let e = elapsed - self.pre;
        let cl = self.cycle_len().max(1);
        let cycle = (e / cl + 1).min(self.cycles);
        let mut t = e % cl;
        let r = self.ratio;
        for (phase, len) in [
            (Phase::Inhale, r.inhale),
            (Phase::Retain, r.retain),
            (Phase::Exhale, r.exhale),
            (Phase::Sustain, r.sustain),
        ] {
            if len == 0 {
                continue;
            }
            if t < len {
                return PhaseInfo {
                    phase,
                    len,
                    left: len - t,
                    cycle,
                };
            }
            t -= len;
        }
        PhaseInfo {
            phase: Phase::Exhale,
            len: r.exhale,
            left: 0,
            cycle,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RawItem {
    pose: Option<String>,
    breath: Option<String>,
    seconds: Option<u32>,
    side: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawRoutine {
    id: String,
    title: String,
    desc: String,
    items: Vec<RawItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub fn label(self) -> &'static str {
        match self {
            Side::Left => "左側",
            Side::Right => "右側",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StepKind {
    Pose { idx: usize, side: Option<Side> },
    Breath { idx: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub kind: StepKind,
    pub seconds: u32,
}

impl Step {
    pub fn title(&self) -> String {
        match &self.kind {
            StepKind::Pose { idx, side } => {
                let p = &DATA.poses[*idx];
                match side {
                    Some(s) => format!("{}（{}）", p.zh, s.label()),
                    None => p.zh.clone(),
                }
            }
            StepKind::Breath { idx } => DATA.breathing[*idx].name.clone(),
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Routine {
    pub id: String,
    pub title: String,
    pub desc: String,
    pub steps: Vec<Step>,
}

impl Routine {
    pub fn total(&self) -> u32 {
        self.steps.iter().map(|s| s.seconds).sum()
    }
    pub fn minutes(&self) -> u32 {
        (self.total() + 30) / 60
    }
    /// 不重複的體式名稱，用在首頁卡片。
    pub fn pose_names(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for s in &self.steps {
            let name = match &s.kind {
                StepKind::Pose { idx, .. } => DATA.poses[*idx].zh.clone(),
                StepKind::Breath { idx } => DATA.breathing[*idx].name.clone(),
            };
            if !out.contains(&name) {
                out.push(name);
            }
        }
        out
    }
}

pub struct Data {
    pub poses: Vec<Pose>,
    pub breathing: Vec<Breathing>,
    pub routines: Vec<Routine>,
}

pub static DATA: LazyLock<Data> = LazyLock::new(|| {
    let mut poses: Vec<Pose> =
        serde_json::from_str(include_str!("../data/poses.json")).expect("poses.json 格式錯誤");
    for p in poses.iter_mut() {
        p.svg = crate::figure::render(&p.figure, p.period);
    }
    let breathing: Vec<Breathing> = serde_json::from_str(include_str!("../data/breathing.json"))
        .expect("breathing.json 格式錯誤");
    let raw: Vec<RawRoutine> = serde_json::from_str(include_str!("../data/routines.json"))
        .expect("routines.json 格式錯誤");

    let routines = raw
        .into_iter()
        .map(|r| {
            let steps = r
                .items
                .iter()
                .filter_map(|it| {
                    if let Some(b) = &it.breath {
                        let idx = breathing.iter().position(|x| &x.id == b)?;
                        let seconds = it.seconds.unwrap_or_else(|| breathing[idx].total());
                        return Some(Step {
                            kind: StepKind::Breath { idx },
                            seconds,
                        });
                    }
                    let id = it.pose.as_ref()?;
                    let idx = poses.iter().position(|x| &x.id == id)?;
                    let side = match it.side.as_deref() {
                        Some("left") => Some(Side::Left),
                        Some("right") => Some(Side::Right),
                        _ => None,
                    };
                    Some(Step {
                        kind: StepKind::Pose { idx, side },
                        seconds: it.seconds.unwrap_or(60),
                    })
                })
                .collect();
            Routine {
                id: r.id,
                title: r.title,
                desc: r.desc,
                steps,
            }
        })
        .collect();

    Data {
        poses,
        breathing,
        routines,
    }
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_loads_and_routine_lengths() {
        let d = &*DATA;
        assert_eq!(d.poses.len(), 10);
        assert_eq!(d.breathing.len(), 3);
        for r in &d.routines {
            // 每個課程的每一步都要成功對應到資料
            assert!(!r.steps.is_empty(), "{} 沒有步驟", r.id);
        }
        let mins: Vec<u32> = d.routines.iter().map(|r| r.minutes()).collect();
        assert_eq!(mins, vec![5, 10, 15]);
    }

    /// `PREVIEW=1 cargo test preview` 會把所有示範圖輸出到 target/figures.html
    #[test]
    fn preview_figures() {
        if std::env::var("PREVIEW").is_err() {
            return;
        }
        let cards: String = DATA
            .poses
            .iter()
            .flat_map(|p| {
                let mut v = vec![format!("<div class=c>{}<p>{}</p></div>", p.svg, p.zh)];
                if p.figure.len() > 1 {
                    for (i, f) in p.figure.iter().enumerate() {
                        let svg = crate::figure::render(std::slice::from_ref(f), p.period);
                        v.push(format!(
                            "<div class=c>{svg}<p>{} 影格 {}</p></div>",
                            p.zh,
                            i + 1
                        ));
                    }
                }
                v
            })
            .collect();
        let html = format!(
            "<!doctype html><meta charset=utf-8><style>body{{background:#0b1020;color:#ddd;font-family:sans-serif;\
             display:grid;grid-template-columns:repeat(2,1fr);gap:8px;margin:8px}}.c{{background:#151b36;border-radius:12px;\
             padding:6px;text-align:center}}svg{{width:100%}}p{{margin:0}}</style>{cards}"
        );
        std::fs::write(
            concat!(env!("CARGO_MANIFEST_DIR"), "/target/figures.html"),
            html,
        )
        .unwrap();
    }

    #[test]
    fn breathing_phases_478() {
        let b = DATA.breathing.iter().find(|b| b.id == "478").unwrap();
        assert_eq!(b.phase_at(0).phase, Phase::Prepare);
        assert_eq!(b.phase_at(5).phase, Phase::Inhale);
        assert_eq!(b.phase_at(5).left, 4);
        assert_eq!(b.phase_at(9).phase, Phase::Retain);
        assert_eq!(b.phase_at(16).phase, Phase::Exhale);
        assert_eq!(b.phase_at(16).left, 8);
        assert_eq!(b.phase_at(24).phase, Phase::Inhale);
        assert_eq!(b.phase_at(24).cycle, 2);
    }
}
