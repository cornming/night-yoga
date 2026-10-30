//! 要念的話。每一句有固定的 id，對應預錄語音檔 `voice/<id>.mp3`；
//! 沒有那個檔案時，改用瀏覽器內建語音合成念 `text`。
//! 語音稿 `tools/voice/script.csv` 由 [`catalog`] 決定，單元測試會檢查兩邊一致。

use crate::data::{DATA, Phase, Side, StepKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub id: String,
    pub text: String,
}

fn side_id(side: Side) -> &'static str {
    match side {
        Side::Left => "left",
        Side::Right => "right",
    }
}

/// 體式提示：「體式名稱（，左側／右側）。口訣」。
pub fn pose(idx: usize, side: Option<Side>) -> Line {
    let p = &DATA.poses[idx];
    match side {
        Some(sd) => Line {
            id: format!("pose_{}_{}", p.id, side_id(sd)),
            text: format!("{}，{}。{}", p.zh, sd.label(), p.cue),
        },
        None => Line {
            id: format!("pose_{}", p.id),
            text: format!("{}。{}", p.zh, p.cue),
        },
    }
}

pub fn breath(idx: usize) -> Line {
    let b = &DATA.breathing[idx];
    Line {
        id: format!("breath_{}", b.id),
        text: format!("{}，跟著圓圈慢慢呼吸。", b.name),
    }
}

/// 呼吸口令。兩種「停住」共用同一句。
pub fn phase(phase: Phase) -> Line {
    let id = match phase {
        Phase::Prepare => "prepare",
        Phase::Inhale => "inhale",
        Phase::Retain | Phase::Sustain => "hold",
        Phase::Exhale => "exhale",
    };
    Line {
        id: format!("phase_{id}"),
        text: phase.label().to_string(),
    }
}

pub fn ending() -> Line {
    Line {
        id: "end".to_string(),
        text: "練習結束，晚安，好好睡一覺。".to_string(),
    }
}

/// 全部語音稿（依課程出現的順序、不重複）。
pub fn catalog() -> Vec<Line> {
    let mut out: Vec<Line> = Vec::new();
    let mut add = |line: Line| {
        if !out.iter().any(|l| l.id == line.id) {
            out.push(line);
        }
    };
    for r in &DATA.routines {
        for s in &r.steps {
            match s.kind {
                StepKind::Pose { idx, side } => add(pose(idx, side)),
                StepKind::Breath { idx } => add(breath(idx)),
            }
        }
    }
    for p in [Phase::Inhale, Phase::Retain, Phase::Exhale] {
        add(phase(p));
    }
    add(ending());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 解析 tools/voice/script.csv：每行 `id,"text"`。
    fn script() -> Vec<(String, String)> {
        include_str!("../tools/voice/script.csv")
            .lines()
            .skip(1)
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let (id, text) = l.split_once(',').expect("script.csv 每行要有逗號");
                (id.to_string(), text.trim_matches('"').to_string())
            })
            .collect()
    }

    #[test]
    fn script_file_matches_the_catalog() {
        let expected: Vec<(String, String)> =
            catalog().into_iter().map(|l| (l.id, l.text)).collect();
        assert_eq!(
            script(),
            expected,
            "改了要念的話，要同步更新 tools/voice/script.csv"
        );
    }

    /// `WRITE_SCRIPT=1 cargo test write_script` 依 [`catalog`] 重新寫出 tools/voice/script.csv
    #[test]
    fn write_script() {
        if std::env::var("WRITE_SCRIPT").is_err() {
            return;
        }
        let mut out = String::from("id,text\n");
        for l in catalog() {
            out.push_str(&format!("{},\"{}\"\n", l.id, l.text));
        }
        std::fs::write(
            concat!(env!("CARGO_MANIFEST_DIR"), "/tools/voice/script.csv"),
            out,
        )
        .expect("寫入 script.csv 失敗");
    }

    #[test]
    fn ids_are_unique_and_file_safe() {
        let cat = catalog();
        for l in &cat {
            assert!(
                l.id.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
                "{} 不能當檔名",
                l.id
            );
            assert_eq!(cat.iter().filter(|x| x.id == l.id).count(), 1);
        }
    }

    #[test]
    fn both_holds_share_one_clip() {
        assert_eq!(phase(Phase::Retain), phase(Phase::Sustain));
    }
}
