//! 動作中的引導：提醒、倒數、換邊、呼吸輪次。
//!
//! 全部是純函式：給「這一步的資料、還剩幾秒」，回答「這一秒要不要說話、說什麼」，
//! 所以不用開瀏覽器就能用單元測試驗證節奏。要念的話和 [`crate::voice`] 一樣有固定 id，
//! 會一起寫進 tools/voice/script.csv，之後可以錄成自然語音。

use crate::data::{DATA, Pose, Side, Step, StepKind};
use crate::voice::Line;

/// 動作開始後多久說第一句提醒（開頭那句口訣大約要念 6 到 8 秒）。
const FIRST_TIP_AT: u32 = 15;
/// 之後每隔幾秒說一句。睡前不要一直講話，所以疏一點。
const TIP_EVERY: u32 = 25;
/// 結尾前這幾秒不說提醒，留給「換邊」和倒數。
const QUIET_BEFORE_END: u32 = 10;
/// 剩幾秒開始倒數；太短的動作不倒數。
const COUNTDOWN_FROM: u32 = 3;
const COUNTDOWN_MIN_STEP: u32 = 15;
/// 左右換邊：剩幾秒時提醒「準備換邊」。
const SWITCH_NOTICE_AT: u32 = 6;
const SWITCH_MIN_STEP: u32 = 20;

/// 穿插在動作提醒之間、輪流說的放鬆提示。
const REST: [&str; 4] = [
    "跟著呼吸，慢慢吸氣，慢慢吐氣。",
    "肩膀放鬆，下巴也放鬆。",
    "呼吸保持緩慢，不用急。",
    "讓身體一點一點變沉。",
];

fn count_line(n: u32) -> Line {
    let text = match n {
        1 => "一",
        2 => "二",
        _ => "三",
    };
    Line {
        id: format!("count_{n}"),
        text: text.to_string(),
    }
}

fn switch_line() -> Line {
    Line {
        id: "switch_side".to_string(),
        text: "準備換邊。".to_string(),
    }
}

fn rest_line(i: usize) -> Line {
    Line {
        id: format!("rest_{}", i + 1),
        text: REST[i % REST.len()].to_string(),
    }
}

/// 一個體式在動作中會輪流說的提醒：注意事項（有的話放第一句，跟安全有關，短的動作也要聽得到）、
/// 該體式自己的步驟（第一步開頭已經念過，所以從第二步起），與放鬆提示交錯。
/// 內容都來自 data/poses.json，不另外編造。
fn tips(p: &Pose) -> Vec<Line> {
    let mut mine: Vec<Line> = p
        .steps
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, t)| Line {
            id: format!("tip_{}_{}", p.id, i),
            text: t.clone(),
        })
        .collect();
    if let Some(c) = &p.caution {
        mine.insert(
            0,
            Line {
                id: format!("caution_{}", p.id),
                text: c.clone(),
            },
        );
    }
    let mut out = Vec::new();
    let mut mine = mine.into_iter();
    let mut rest = (0..REST.len()).map(rest_line);
    loop {
        let (a, b) = (mine.next(), rest.next());
        if a.is_none() && b.is_none() {
            break;
        }
        out.extend(a);
        out.extend(b);
    }
    out
}

/// 下一步是同一個體式的另一側，就是要換邊。
fn is_switch(idx: usize, side: Option<Side>, next: Option<&Step>) -> bool {
    let Some(side) = side else { return false };
    matches!(
        next,
        Some(Step { kind: StepKind::Pose { idx: ni, side: Some(ns) }, .. }) if *ni == idx && *ns != side
    )
}

/// 體式進行中：這一秒要不要說話。`left` 是畫面上剛剛顯示的剩餘秒數。
/// 優先順序：倒數 > 換邊提醒 > 一般提醒。
pub fn pose_cue(step: &Step, next: Option<&Step>, left: u32) -> Option<Line> {
    let StepKind::Pose { idx, side } = &step.kind else {
        return None;
    };
    let total = step.seconds;
    if left == 0 || left > total {
        return None;
    }
    if total >= COUNTDOWN_MIN_STEP && left <= COUNTDOWN_FROM {
        return Some(count_line(left));
    }
    if total >= SWITCH_MIN_STEP && left == SWITCH_NOTICE_AT && is_switch(*idx, *side, next) {
        return Some(switch_line());
    }
    let elapsed = total - left;
    if left >= QUIET_BEFORE_END
        && elapsed >= FIRST_TIP_AT
        && (elapsed - FIRST_TIP_AT) % TIP_EVERY == 0
    {
        // 右側從下一句開始，不要左右兩邊聽到一模一樣的話
        let n = ((elapsed - FIRST_TIP_AT) / TIP_EVERY) as usize + usize::from(*side == Some(Side::Right));
        let list = tips(&DATA.poses[*idx]);
        return Some(list[n % list.len()].clone());
    }
    None
}

fn zh_num(n: u32) -> String {
    const D: [&str; 10] = ["零", "一", "二", "三", "四", "五", "六", "七", "八", "九"];
    D.get(n as usize).map_or_else(|| n.to_string(), |s| s.to_string())
}

/// 呼吸步驟每一輪開始時的口令。第一輪照舊只念「吸氣」，之後念「第二輪」…，最後一輪念「最後一輪」。
pub fn breath_cycle_line(breath_idx: usize, cycle: u32) -> Option<Line> {
    let b = &DATA.breathing[breath_idx];
    if cycle < 2 || cycle > b.cycles {
        return None;
    }
    if cycle == b.cycles {
        return Some(Line {
            id: "cycle_last".to_string(),
            text: "最後一輪，吸氣。".to_string(),
        });
    }
    Some(Line {
        id: format!("cycle_{cycle}"),
        text: format!("第{}輪，吸氣。", zh_num(cycle)),
    })
}

/// 引導會用到的全部語音稿（不重複），接在 [`crate::voice::catalog`] 後面。
pub fn catalog() -> Vec<Line> {
    let mut out: Vec<Line> = Vec::new();
    let mut add = |line: Line| {
        if !out.iter().any(|l| l.id == line.id) {
            out.push(line);
        }
    };
    for n in (1..=COUNTDOWN_FROM).rev() {
        add(count_line(n));
    }
    add(switch_line());
    for r in &DATA.routines {
        for s in &r.steps {
            if let StepKind::Pose { idx, .. } = s.kind {
                for l in tips(&DATA.poses[idx]) {
                    add(l);
                }
            }
        }
    }
    for i in 0..DATA.breathing.len() {
        for c in 2..=DATA.breathing[i].cycles {
            if let Some(l) = breath_cycle_line(i, c) {
                add(l);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose_step(idx: usize, side: Option<Side>, seconds: u32) -> Step {
        Step {
            kind: StepKind::Pose { idx, side },
            seconds,
        }
    }

    fn pose_idx(id: &str) -> usize {
        DATA.poses.iter().position(|p| p.id == id).expect("找不到體式")
    }

    /// 把一步從頭走到尾，收集 (已過秒數, 剩餘秒數, 要說的話)。
    fn timeline(step: &Step, next: Option<&Step>) -> Vec<(u32, u32, Line)> {
        (1..=step.seconds)
            .rev()
            .filter_map(|left| {
                pose_cue(step, next, left).map(|l| (step.seconds - left, left, l))
            })
            .collect()
    }

    #[test]
    fn counts_down_the_last_three_seconds() {
        let s = pose_step(pose_idx("child"), None, 60);
        let ids: Vec<String> = [3, 2, 1]
            .iter()
            .map(|&l| pose_cue(&s, None, l).unwrap().id)
            .collect();
        assert_eq!(ids, ["count_3", "count_2", "count_1"]);
        assert_eq!(pose_cue(&s, None, 0), None, "剩 0 秒已經換下一步，不說話");
    }

    #[test]
    fn short_steps_have_no_countdown() {
        let s = pose_step(pose_idx("child"), None, 10);
        assert!(timeline(&s, None).is_empty());
    }

    #[test]
    fn stays_quiet_at_the_start_then_speaks_sparsely() {
        let s = pose_step(pose_idx("cat-cow"), None, 120);
        let tips: Vec<u32> = timeline(&s, None)
            .into_iter()
            .filter(|(_, left, _)| *left > COUNTDOWN_FROM)
            .map(|(e, _, _)| e)
            .collect();
        assert_eq!(tips, [15, 40, 65, 90], "開頭 15 秒安靜，之後約每 25 秒一句");
        assert!(tips.windows(2).all(|w| w[1] - w[0] >= TIP_EVERY));
    }

    #[test]
    fn the_second_side_does_not_repeat_the_first_sides_reminder() {
        for r in &DATA.routines {
            for (i, s) in r.steps.iter().enumerate() {
                let (Some(n), StepKind::Pose { side: Some(Side::Left), .. }) = (r.steps.get(i + 1), &s.kind) else { continue };
                if !is_switch(match s.kind { StepKind::Pose { idx, .. } => idx, _ => 0 }, Some(Side::Left), Some(n)) {
                    continue;
                }
                let first = |st: &Step| timeline(st, None).into_iter().find(|(_, left, _)| *left > COUNTDOWN_FROM).map(|(_, _, l)| l.id);
                if s.seconds >= 20 && n.seconds >= 20 {
                    assert_ne!(first(s), first(n), "{} 第 {} 步左右兩側的第一句提醒一樣", r.id, i + 1);
                }
            }
        }
    }

    #[test]
    fn a_caution_is_the_first_reminder_so_even_short_poses_say_it() {
        let neck = pose_idx("neck");
        assert!(DATA.poses[neck].caution.is_some());
        let left = pose_step(neck, Some(Side::Left), 30);
        let first = timeline(&left, None).into_iter().find(|(_, l, _)| *l > COUNTDOWN_FROM).unwrap();
        assert_eq!(first.2.id, "caution_neck");
    }

    #[test]
    fn thirty_second_poses_still_get_one_reminder() {
        let s = pose_step(pose_idx("neck"), Some(Side::Left), 30);
        let spoken: Vec<_> = timeline(&s, None)
            .into_iter()
            .filter(|(_, left, _)| *left > COUNTDOWN_FROM)
            .collect();
        assert_eq!(spoken.len(), 1);
        assert_eq!(spoken[0].0, 15);
    }

    #[test]
    fn never_speaks_a_tip_in_the_last_ten_seconds() {
        for idx in 0..DATA.poses.len() {
            for secs in 15..=150 {
                for (_, left, line) in timeline(&pose_step(idx, None, secs), None) {
                    let is_count = line.id.starts_with("count_");
                    assert!(is_count || left >= QUIET_BEFORE_END, "{} {}s 剩 {left}", line.id, secs);
                }
            }
        }
    }

    #[test]
    fn switch_notice_only_when_the_next_step_is_the_other_side() {
        let neck = pose_idx("neck");
        let left = pose_step(neck, Some(Side::Left), 30);
        let right = pose_step(neck, Some(Side::Right), 30);
        let other = pose_step(pose_idx("cat-cow"), None, 60);
        assert_eq!(pose_cue(&left, Some(&right), 6).unwrap().id, "switch_side");
        assert_ne!(pose_cue(&right, Some(&other), 6).map(|l| l.id), Some("switch_side".into()));
        assert_ne!(pose_cue(&left, Some(&left), 6).map(|l| l.id), Some("switch_side".into()));
        let no_side = pose_step(pose_idx("child"), None, 60);
        assert_ne!(pose_cue(&no_side, Some(&no_side), 6).map(|l| l.id), Some("switch_side".into()));
    }

    #[test]
    fn real_routines_switch_exactly_at_the_left_to_right_boundary() {
        for r in &DATA.routines {
            for (i, s) in r.steps.iter().enumerate() {
                let StepKind::Pose { side, .. } = s.kind else { continue };
                let next = r.steps.get(i + 1);
                let switches = timeline(s, next).iter().any(|(_, _, l)| l.id == "switch_side");
                let expect = side == Some(Side::Left)
                    && next.is_some_and(|n| matches!(n.kind, StepKind::Pose { side: Some(Side::Right), .. }));
                assert_eq!(switches, expect, "{} 第 {} 步", r.id, i + 1);
            }
        }
    }

    #[test]
    fn tips_use_the_poses_own_text_and_never_repeat_back_to_back() {
        for p in &DATA.poses {
            let t = tips(p);
            assert!(!t.is_empty());
            assert!(t.windows(2).all(|w| w[0].id != w[1].id), "{}", p.id);
            for l in &t {
                let ok = l.id.starts_with("rest_")
                    || p.steps.iter().any(|s| *s == l.text)
                    || p.caution.as_deref() == Some(l.text.as_str());
                assert!(ok, "{} 的提醒不是來自資料：{}", p.id, l.text);
            }
        }
    }

    #[test]
    fn tips_are_short_enough_to_finish_before_the_end() {
        // 內建語音大約每秒 4 個字；最晚的提醒距離結尾 10 秒，再留 3 秒倒數前的空檔
        for p in &DATA.poses {
            for l in tips(p) {
                assert!(l.text.chars().count() <= 28, "{} 太長：{}", l.id, l.text);
            }
        }
    }

    #[test]
    fn every_spoken_line_is_in_the_catalog() {
        let cat: Vec<String> = catalog().into_iter().map(|l| l.id).collect();
        for r in &DATA.routines {
            for (i, s) in r.steps.iter().enumerate() {
                for (_, _, l) in timeline(s, r.steps.get(i + 1)) {
                    assert!(cat.contains(&l.id), "{} 不在語音稿裡", l.id);
                }
            }
        }
        for b in 0..DATA.breathing.len() {
            for c in 1..=DATA.breathing[b].cycles {
                if let Some(l) = breath_cycle_line(b, c) {
                    assert!(cat.contains(&l.id), "{} 不在語音稿裡", l.id);
                }
            }
        }
    }

    #[test]
    fn breath_announces_rounds_and_the_last_one() {
        for (i, b) in DATA.breathing.iter().enumerate() {
            assert_eq!(breath_cycle_line(i, 0), None);
            assert_eq!(breath_cycle_line(i, 1), None, "第一輪照舊只念吸氣");
            assert_eq!(breath_cycle_line(i, b.cycles + 1), None);
            assert_eq!(breath_cycle_line(i, b.cycles).unwrap().id, "cycle_last");
            assert_eq!(breath_cycle_line(i, 2).unwrap().text.starts_with("第二輪"), b.cycles > 2);
        }
    }

    #[test]
    fn non_pose_steps_get_no_pose_cue() {
        let s = Step {
            kind: StepKind::Breath { idx: 0 },
            seconds: 60,
        };
        assert!(timeline_any(&s).is_empty());
    }

    fn timeline_any(s: &Step) -> Vec<Line> {
        (1..=s.seconds).filter_map(|l| pose_cue(s, None, l)).collect()
    }
}
