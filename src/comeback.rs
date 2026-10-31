//! 讓人願意回來的小功能：里程碑、回歸提示、行事曆提醒檔，以及「第幾次完成」的匿名分級。
//! 全部是純函式，不碰瀏覽器，所以能用單元測試驗證。晚安瑜伽與 SlowJog 網頁版共用同一份寫法。

/// 連續天數的里程碑鼓勵。只在剛好達到的那一天說，而且只用一句平實的話，不誇大。
pub fn milestone(streak: u32) -> Option<&'static str> {
    match streak {
        3 => Some("連續 3 天了，好的開始。"),
        7 => Some("連續一週了，給自己的時間一直都在。"),
        14 => Some("連續兩週了。"),
        30 => Some("連續 30 天了。"),
        _ => None,
    }
}

/// 隔了幾天沒來，回來時的一句話：不責備，只把重新開始的門檻放低。
/// `days_since_last` 是距離上次練習幾天（今天練過是 0），從沒練過是 None。
pub fn welcome_back(days_since_last: Option<u32>, how: &str) -> Option<String> {
    match days_since_last {
        Some(d) if d >= 3 => Some(format!("歡迎回來。{how}")),
        _ => None,
    }
}

/// 同一個瀏覽器的第幾次完成，只分四級再回報（1、2、3、4+），不送確切次數。
/// 每個瀏覽器的「第 1 次」只會出現一次，所以第 1 次的筆數約等於不同的手機數，
/// 「第 2 次 ÷ 第 1 次」就是回訪率；4 以上不再細分。
pub fn nth_bucket(completions: u32) -> &'static str {
    match completions {
        0 | 1 => "1",
        2 => "2",
        3 => "3",
        _ => "4+",
    }
}

/// 回報給統計的內容：作品、課程代號、完成當下的小時、第幾次完成的分級。沒有任何能識別個人的資料。
pub fn completion_body(app: &str, course: &str, hour: u32, nth: &str) -> String {
    format!(r#"{{"app":"{app}","course":"{course}","hour":{hour},"nth":"{nth}"}}"#)
}

/// 解析 `<input type="time">` 的值（"HH:MM"，有些瀏覽器會多帶秒數）。
pub fn parse_hhmm(s: &str) -> Option<(u32, u32)> {
    let (h, m) = s.trim().split_once(':')?;
    let m = m.get(..2).unwrap_or(m);
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some((h, m))
}

fn ics_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
}

/// 每 75 個位元組折一行（RFC 5545），而且不會切在多位元組字元中間。
fn fold(line: &str) -> String {
    let mut out = String::new();
    let mut bytes = 0usize;
    for ch in line.chars() {
        let n = ch.len_utf8();
        if bytes + n > 75 {
            out.push_str("\r\n ");
            bytes = 1;
        }
        out.push(ch);
        bytes += n;
    }
    out
}

/// 每天重複的行事曆提醒檔（.ics）。時間不指定時區，所以會跟著手機目前所在的時區；
/// 同一個作品用固定的 UID，再加一次只會更新原本那筆，不會重複。
/// `ymd` 是第一天的日期（YYYYMMDD），`tag` 用來區分不同作品。
pub fn reminder_ics(tag: &str, title: &str, description: &str, url: &str, ymd: &str, hh: u32, mm: u32) -> String {
    let lines = [
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        "PRODID:-//cornming.github.io//reminder//TW".to_string(),
        "CALSCALE:GREGORIAN".to_string(),
        "BEGIN:VEVENT".to_string(),
        format!("UID:{tag}@cornming.github.io"),
        format!("DTSTAMP:{ymd}T000000Z"),
        format!("DTSTART:{ymd}T{hh:02}{mm:02}00"),
        "DURATION:PT10M".to_string(),
        "RRULE:FREQ=DAILY".to_string(),
        format!("SUMMARY:{}", ics_escape(title)),
        format!("DESCRIPTION:{}\\n{}", ics_escape(description), ics_escape(url)),
        format!("URL:{url}"),
        "BEGIN:VALARM".to_string(),
        "TRIGGER:PT0M".to_string(),
        "ACTION:DISPLAY".to_string(),
        format!("DESCRIPTION:{}", ics_escape(title)),
        "END:VALARM".to_string(),
        "END:VEVENT".to_string(),
        "END:VCALENDAR".to_string(),
    ];
    let mut out = lines.iter().map(|l| fold(l)).collect::<Vec<_>>().join("\r\n");
    out.push_str("\r\n");
    out
}

fn pct(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// 「用 Google 日曆加入」的連結：不用下載檔案，手機和電腦都能直接開。
/// 時間不帶時區，Google 會用使用者日曆的時區。
pub fn google_calendar_url(title: &str, details: &str, ymd: &str, hh: u32, mm: u32) -> String {
    let end = (hh * 60 + mm + 10).min(23 * 60 + 59);
    format!(
        "https://calendar.google.com/calendar/render?action=TEMPLATE&text={}&details={}&dates={ymd}T{hh:02}{mm:02}00/{ymd}T{:02}{:02}00&recur={}",
        pct(title),
        pct(details),
        end / 60,
        end % 60,
        pct("RRULE:FREQ=DAILY"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn milestones_only_on_the_exact_day() {
        let hit: Vec<u32> = (0..=60).filter(|n| milestone(*n).is_some()).collect();
        assert_eq!(hit, [3, 7, 14, 30]);
    }

    #[test]
    fn milestone_text_stays_plain() {
        for n in [3, 7, 14, 30] {
            let t = milestone(n).unwrap();
            assert!(t.chars().count() <= 20, "{t}");
            assert!(!t.contains('！'), "{t}");
        }
    }

    #[test]
    fn welcome_back_after_three_days_without_blame() {
        assert_eq!(welcome_back(None, "x"), None, "從沒練過的人不是「回來」");
        assert_eq!(welcome_back(Some(0), "x"), None);
        assert_eq!(welcome_back(Some(2), "x"), None);
        assert_eq!(welcome_back(Some(3), "先做最短的就好。").unwrap(), "歡迎回來。先做最短的就好。");
        assert!(welcome_back(Some(40), "x").is_some());
    }

    #[test]
    fn nth_buckets_never_reveal_the_exact_count() {
        let b: Vec<&str> = [0, 1, 2, 3, 4, 5, 100, u32::MAX].iter().map(|n| nth_bucket(*n)).collect();
        assert_eq!(b, ["1", "1", "2", "3", "4+", "4+", "4+", "4+"]);
    }

    #[test]
    fn completion_body_is_valid_json_with_only_the_expected_fields() {
        let v: serde_json::Value = serde_json::from_str(&completion_body("night-yoga", "quick", 22, "2")).unwrap();
        let mut keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(keys, ["app", "course", "hour", "nth"]);
        assert_eq!(v["hour"], 22);
        assert_eq!(v["nth"], "2");
    }

    #[test]
    fn parses_time_inputs() {
        assert_eq!(parse_hhmm("22:00"), Some((22, 0)));
        assert_eq!(parse_hhmm("06:30:15"), Some((6, 30)));
        assert_eq!(parse_hhmm(" 7:05 "), Some((7, 5)));
        for bad in ["", "24:00", "12:60", "ab:cd", "1230", "12:"] {
            assert_eq!(parse_hhmm(bad), None, "{bad}");
        }
    }

    #[test]
    fn ics_is_a_daily_floating_time_event_with_an_alarm() {
        let s = reminder_ics("night-yoga", "晚安瑜伽：睡前 5 分鐘", "做一次睡前伸展，慢慢把今天放下", "https://cornming.github.io/night-yoga/", "20261101", 22, 0);
        assert!(s.starts_with("BEGIN:VCALENDAR\r\n") && s.ends_with("END:VCALENDAR\r\n"));
        for must in ["RRULE:FREQ=DAILY", "DTSTART:20261101T220000", "BEGIN:VALARM", "TRIGGER:PT0M", "UID:night-yoga@cornming.github.io", "URL:https://cornming.github.io/night-yoga/"] {
            assert!(s.contains(must), "缺少 {must}");
        }
        assert!(!s.contains("TZID") && !s.contains("DTSTART:20261101T220000Z"), "不指定時區，才會跟著手機");
        assert!(!s.contains('\n') || s.matches('\n').count() == s.matches("\r\n").count(), "換行一律 CRLF");
    }

    #[test]
    fn ics_lines_fold_at_75_bytes_and_unfold_back() {
        let title = "晚安瑜伽：睡前五分鐘，把今天放下，慢慢呼吸，慢慢伸展，然後安心睡覺，明天見";
        let s = reminder_ics("t", title, "d", "https://x/", "20261101", 6, 30);
        assert!(s.split("\r\n").all(|l| l.len() <= 75), "有超過 75 位元組的行");
        let unfolded = s.replace("\r\n ", "");
        assert!(unfolded.contains(&format!("SUMMARY:{}", ics_escape(title))), "折行後要能還原成原句");
    }

    #[test]
    fn ics_escapes_text_values() {
        let s = reminder_ics("t", "a,b;c\\d", "x\ny", "https://x/", "20261101", 6, 30);
        assert!(s.contains("SUMMARY:a\\,b\\;c\\\\d"));
        assert!(s.contains("DESCRIPTION:x\\ny\\nhttps://x/"));
    }

    #[test]
    fn google_url_encodes_text_and_clamps_the_end_time() {
        let u = google_calendar_url("晚安瑜伽：睡前 5 分鐘", "網址 https://a/b?c=d", "20261101", 22, 0);
        assert!(u.starts_with("https://calendar.google.com/calendar/render?action=TEMPLATE&text=%E6%99%9A"));
        assert!(u.contains("dates=20261101T220000/20261101T221000"));
        assert!(u.contains("recur=RRULE%3AFREQ%3DDAILY"));
        assert!(!u.contains(' ') && !u.contains('：'), "不能有沒編碼的字元");
        let late = google_calendar_url("t", "d", "20261101", 23, 55);
        assert!(late.contains("dates=20261101T235500/20261101T235900"), "{late}");
    }
}
