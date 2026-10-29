//! 體式示範圖：用「骨架＋關節角度」描述動作，依固定的人體比例算出四肢位置，
//! 再畫成有粗細、前後層次的人形插畫。兩個影格的動作（例如貓牛式）會產生 SMIL 動畫。
//!
//! 座標系：viewBox 0 0 240 150，y 往下；角度以度為單位，0° 朝右、90° 朝下。

use serde::Deserialize;
use std::fmt::Write;

// ── 人體比例 ─────────────────────────────
const TORSO: f32 = 50.0;
const NECK: f32 = 10.0;
const HEAD_R: f32 = 11.0;
const UPPER_ARM: f32 = 28.0;
const FOREARM: f32 = 26.0;
const THIGH: f32 = 40.0;
const SHIN: f32 = 38.0;
const FOOT: f32 = 11.0;
const SHOULDER_HALF: f32 = 17.0;
const HIP_HALF: f32 = 11.0;

// ── 配色（深色背景專用）──────────────────
const SKIN: &str = "#f1d7c2";
const SKIN_FAR: &str = "#c4a28c";
const SHIRT: &str = "#9b91dc";
const PANTS: &str = "#5f57a8";
const PANTS_FAR: &str = "#463f86";
const HAIR: &str = "#2b2642";
const MAT: &str = "#3a4176";
const MAT_TOP: &str = "#252a51";
const MAT_EDGE: &str = "#363d70";
const WALL: &str = "#323865";

pub const GROUND: f32 = 126.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum View {
    /// 側面，人物下方有瑜伽墊
    Side,
    /// 正面
    Front,
    /// 從上往下看，整片瑜伽墊在背景
    Top,
}

/// 一隻手或腳：直接給每一節的角度，或給末端要碰到的點讓程式自動算關節（IK）。
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Limb {
    Reach {
        to: [f32; 2],
        /// 關節往哪一側彎：1 或 -1
        #[serde(default = "one")]
        bend: f32,
        /// 腳掌角度（只用在腳）
        #[serde(default)]
        end: Option<f32>,
    },
    Angles(Vec<f32>),
}

fn one() -> f32 {
    1.0
}
fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct Skeleton {
    pub view: View,
    pub hip: [f32; 2],
    /// 軀幹方向（從髖部指向肩膀）
    pub torso: f32,
    /// 軀幹彎曲量：正值往軀幹的右手垂直方向凸出（側面時通常是往下塌）
    #[serde(default)]
    pub bend: f32,
    /// 頸部方向（從肩膀指向頭）
    pub neck: f32,
    /// [遠側, 近側]；正面／俯視時是 [畫面左側, 畫面右側]
    pub arms: [Limb; 2],
    pub legs: [Limb; 2],
    /// 側面時臉朝哪邊：1 = 右、-1 = 左
    #[serde(default = "one")]
    pub face: f32,
    #[serde(default = "yes")]
    pub bun: bool,
    /// 牆面的 x 座標（靠牆動作）
    #[serde(default)]
    pub wall: Option<f32>,
    /// 側面時把近側的腿畫在身體後面（腿收在身體下方的動作，例如嬰兒式）
    #[serde(default)]
    pub tuck: bool,
}

type P = [f32; 2];

fn dir(deg: f32) -> P {
    let r = deg.to_radians();
    [r.cos(), r.sin()]
}
fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1]]
}
fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1]]
}
fn mul(a: P, k: f32) -> P {
    [a[0] * k, a[1] * k]
}
fn len(a: P) -> f32 {
    (a[0] * a[0] + a[1] * a[1]).sqrt()
}
fn ang(a: P) -> f32 {
    a[1].atan2(a[0]).to_degrees()
}

/// 兩節式 IK：從 root 出發，兩節長度 l1、l2，讓末端盡量碰到 target。
fn two_bone(root: P, l1: f32, l2: f32, target: P, bend: f32) -> (P, P) {
    let v = sub(target, root);
    let d = len(v).clamp((l1 - l2).abs() + 0.01, l1 + l2 - 0.01);
    let base = ang(v);
    let cos_a = ((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d)).clamp(-1.0, 1.0);
    let a = cos_a.acos().to_degrees();
    let joint = add(root, mul(dir(base + bend.signum() * a), l1));
    let end = add(root, mul(dir(base), d));
    (joint, end)
}

/// 單一繪圖元件；兩個影格的元件一一對應，才能做補間動畫。
#[derive(Debug, Clone)]
enum Prim {
    Line {
        a: P,
        b: P,
        w: f32,
        color: &'static str,
    },
    Torso {
        d: String,
        w: f32,
        fill: bool,
    },
    Circle {
        c: P,
        r: f32,
        color: &'static str,
    },
}

fn limb_points(root: P, limb: &Limb, lens: &[f32]) -> Vec<P> {
    match limb {
        Limb::Angles(a) => {
            let mut pts = vec![root];
            let mut cur = root;
            for (i, l) in lens.iter().enumerate() {
                let deg = a
                    .get(i)
                    .copied()
                    .unwrap_or_else(|| *a.last().unwrap_or(&90.0));
                cur = add(cur, mul(dir(deg), *l));
                pts.push(cur);
            }
            pts
        }
        Limb::Reach { to, bend, end } => {
            let (j, e) = two_bone(root, lens[0], lens[1], *to, *bend);
            let mut pts = vec![root, j, e];
            if lens.len() > 2 {
                let deg = end.unwrap_or_else(|| ang(sub(e, j)) + 90.0);
                pts.push(add(e, mul(dir(deg), lens[2])));
            }
            pts
        }
    }
}

fn build(s: &Skeleton) -> Vec<Prim> {
    let hip = s.hip;
    let td = dir(s.torso);
    let shoulder = add(hip, mul(td, TORSO));
    // 軀幹的垂直方向
    let perp = [-td[1], td[0]];
    let mut out = Vec::new();

    let wide = s.view != View::Side;
    let side_off = |i: usize, half: f32| -> P {
        if wide {
            let k = if i == 0 { -half } else { half };
            mul(perp, k)
        } else {
            [0.0, 0.0]
        }
    };

    let arm = |i: usize| -> Vec<Prim> {
        let root = add(shoulder, side_off(i, SHOULDER_HALF));
        let p = limb_points(root, &s.arms[i], &[UPPER_ARM, FOREARM]);
        let far = s.view == View::Side && i == 0;
        let c = if far { SKIN_FAR } else { SKIN };
        vec![
            Prim::Line {
                a: p[0],
                b: p[1],
                w: 10.0,
                color: c,
            },
            Prim::Line {
                a: p[1],
                b: p[2],
                w: 8.5,
                color: c,
            },
            Prim::Circle {
                c: p[2],
                r: 5.2,
                color: c,
            },
        ]
    };
    let leg = |i: usize| -> Vec<Prim> {
        let root = add(hip, side_off(i, HIP_HALF));
        let p = limb_points(root, &s.legs[i], &[THIGH, SHIN, FOOT]);
        let far = s.view == View::Side && i == 0;
        let (c, sk) = if far {
            (PANTS_FAR, SKIN_FAR)
        } else {
            (PANTS, SKIN)
        };
        vec![
            Prim::Line {
                a: p[0],
                b: p[1],
                w: 15.0,
                color: c,
            },
            Prim::Line {
                a: p[1],
                b: p[2],
                w: 11.5,
                color: c,
            },
            Prim::Line {
                a: p[2],
                b: p[3],
                w: 7.0,
                color: sk,
            },
        ]
    };

    // 軀幹
    let torso = if wide {
        let ls = add(shoulder, mul(perp, -SHOULDER_HALF));
        let rs = add(shoulder, mul(perp, SHOULDER_HALF));
        let lh = add(hip, mul(perp, -HIP_HALF));
        let rh = add(hip, mul(perp, HIP_HALF));
        let mid = mul(add(shoulder, hip), 0.5);
        let w = add(mid, mul(perp, -(SHOULDER_HALF + HIP_HALF) * 0.42));
        let e = add(mid, mul(perp, (SHOULDER_HALF + HIP_HALF) * 0.42));
        // 腰身稍微內收
        let d = format!(
            "M{:.1} {:.1} Q{:.1} {:.1} {:.1} {:.1} L{:.1} {:.1} Q{:.1} {:.1} {:.1} {:.1} Z",
            ls[0], ls[1], w[0], w[1], lh[0], lh[1], rh[0], rh[1], e[0], e[1], rs[0], rs[1]
        );
        Prim::Torso {
            d,
            w: 9.0,
            fill: true,
        }
    } else {
        let mid = add(mul(add(shoulder, hip), 0.5), mul(perp, s.bend));
        let d = format!(
            "M{:.1} {:.1} Q{:.1} {:.1} {:.1} {:.1}",
            hip[0], hip[1], mid[0], mid[1], shoulder[0], shoulder[1]
        );
        Prim::Torso {
            d,
            w: 26.0,
            fill: false,
        }
    };

    // 頭與頭髮
    let nd = dir(s.neck);
    let head = add(shoulder, mul(nd, NECK + HEAD_R));
    let back = match s.view {
        View::Side => dir(s.neck - 62.0 * s.face),
        _ => nd,
    };
    let neck = Prim::Line {
        a: shoulder,
        b: add(shoulder, mul(nd, NECK + 2.0)),
        w: 9.0,
        color: SKIN,
    };
    let hair = Prim::Circle {
        c: add(head, mul(back, 2.2)),
        r: HEAD_R + 0.6,
        color: HAIR,
    };
    let bun = Prim::Circle {
        c: add(head, mul(back, HEAD_R + 1.5)),
        r: 6.0,
        color: HAIR,
    };
    let head_c = Prim::Circle {
        c: head,
        r: HEAD_R,
        color: SKIN,
    };
    let mut head_parts = vec![neck, hair];
    if s.bun {
        head_parts.push(bun);
    }
    head_parts.push(head_c);

    match s.view {
        View::Side => {
            out.extend(leg(0));
            out.extend(arm(0));
            if s.tuck {
                out.extend(leg(1));
            }
            out.push(torso);
            out.extend(head_parts);
            if !s.tuck {
                out.extend(leg(1));
            }
            out.extend(arm(1));
        }
        View::Front => {
            out.extend(leg(0));
            out.extend(leg(1));
            out.push(torso);
            out.extend(head_parts);
            out.extend(arm(0));
            out.extend(arm(1));
        }
        View::Top => {
            out.extend(arm(0));
            out.extend(arm(1));
            out.extend(leg(0));
            out.extend(leg(1));
            out.push(torso);
            out.extend(head_parts);
        }
    }
    out
}

fn fmt_p(v: f32) -> String {
    format!("{v:.1}")
}

/// 兩個影格的屬性值不同時，加上來回的 SMIL 補間動畫。
fn anim(out: &mut String, name: &str, a: &str, b: &str, period: f32) {
    if a != b {
        let _ = write!(
            out,
            "<animate attributeName=\"{name}\" values=\"{a};{b};{a}\" dur=\"{period}s\" \
             keyTimes=\"0;0.5;1\" calcMode=\"spline\" \
             keySplines=\"0.45 0 0.55 1;0.45 0 0.55 1\" repeatCount=\"indefinite\"/>"
        );
    }
}

fn render_prim(out: &mut String, p: &Prim, q: Option<&Prim>, period: f32) {
    match (p, q) {
        (Prim::Line { a, b, w, color }, q) => {
            let q = match q {
                Some(Prim::Line { a, b, .. }) => Some((*a, *b)),
                _ => None,
            };
            let vals = [a[0], a[1], b[0], b[1]].map(fmt_p);
            let names = ["x1", "y1", "x2", "y2"];
            out.push_str("<line");
            for (n, v) in names.iter().zip(vals.iter()) {
                let _ = write!(out, " {n}=\"{v}\"");
            }
            let _ = write!(
                out,
                " stroke=\"{color}\" stroke-width=\"{w}\" stroke-linecap=\"round\">"
            );
            if let Some((qa, qb)) = q {
                let qv = [qa[0], qa[1], qb[0], qb[1]].map(fmt_p);
                for i in 0..4 {
                    anim(out, names[i], &vals[i], &qv[i], period);
                }
            }
            out.push_str("</line>");
        }
        (Prim::Torso { d, w, fill }, q) => {
            let fill_attr = if *fill { SHIRT } else { "none" };
            let _ = write!(
                out,
                "<path d=\"{d}\" fill=\"{fill_attr}\" stroke=\"{SHIRT}\" stroke-width=\"{w}\" \
                 stroke-linecap=\"round\" stroke-linejoin=\"round\">"
            );
            if let Some(Prim::Torso { d: qd, .. }) = q {
                anim(out, "d", d, qd, period);
            }
            out.push_str("</path>");
        }
        (Prim::Circle { c, r, color }, q) => {
            let (cx, cy) = (fmt_p(c[0]), fmt_p(c[1]));
            let _ = write!(
                out,
                "<circle cx=\"{cx}\" cy=\"{cy}\" r=\"{r}\" fill=\"{color}\">"
            );
            if let Some(Prim::Circle { c: qc, .. }) = q {
                anim(out, "cx", &cx, &fmt_p(qc[0]), period);
                anim(out, "cy", &cy, &fmt_p(qc[1]), period);
            }
            out.push_str("</circle>");
        }
    }
}

/// 把一或兩個影格畫成完整的 SVG 字串。
pub fn render(frames: &[Skeleton], period: f32) -> String {
    let Some(first) = frames.first() else {
        return String::new();
    };
    let a = build(first);
    let b = frames.get(1).map(build).filter(|b| b.len() == a.len());

    let mut out = String::from(
        "<svg class=\"figure\" viewBox=\"0 0 240 150\" xmlns=\"http://www.w3.org/2000/svg\" aria-hidden=\"true\">",
    );
    match first.view {
        View::Top => {
            let _ = write!(
                out,
                "<rect x=\"14\" y=\"10\" width=\"212\" height=\"130\" rx=\"12\" fill=\"{MAT_TOP}\" stroke=\"{MAT_EDGE}\" stroke-width=\"2\"/>"
            );
        }
        _ => {
            let _ = write!(
                out,
                "<ellipse cx=\"120\" cy=\"{:.0}\" rx=\"104\" ry=\"7\" fill=\"#000\" opacity=\"0.18\"/>\
                 <rect x=\"10\" y=\"{GROUND}\" width=\"220\" height=\"6\" rx=\"3\" fill=\"{MAT}\"/>",
                GROUND + 8.0
            );
        }
    }
    if let Some(x) = first.wall {
        let _ = write!(
            out,
            "<rect x=\"{x}\" y=\"8\" width=\"12\" height=\"{:.0}\" rx=\"3\" fill=\"{WALL}\"/>",
            GROUND + 6.0 - 8.0
        );
    }
    for (i, p) in a.iter().enumerate() {
        render_prim(&mut out, p, b.as_ref().map(|b| &b[i]), period);
    }
    out.push_str("</svg>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ik_reaches_target_when_possible() {
        let (_, end) = two_bone([0.0, 0.0], 28.0, 26.0, [30.0, 20.0], 1.0);
        assert!((end[0] - 30.0).abs() < 0.1 && (end[1] - 20.0).abs() < 0.1);
    }
}
