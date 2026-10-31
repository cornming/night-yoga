//! 辨識「從 LINE 內建瀏覽器開啟」。LINE 內建瀏覽器常常播不出語音，要請使用者改用手機預設的瀏覽器。
//! 全部是純函式，不碰瀏覽器，所以能用單元測試驗證。

/// LINE 內建瀏覽器的 userAgent 結尾會有 `Line/<版本>` 這個詞（iPhone 與 Android 都有，
/// Android 有時還會再接 `/LIAPP`）。照空白切成一個個詞，只認開頭剛好是 `Line/` 的，
/// 這樣 `Deadline/1.0`、`Online/2` 這種名字不會誤判。
pub fn is_line_browser(user_agent: &str) -> bool {
    user_agent.split_whitespace().any(|token| token.starts_with("Line/"))
}

/// LINE 認得的網址參數：內建瀏覽器載入帶有 `openExternalBrowser=1` 的網址時，
/// 會改交給手機的預設瀏覽器開啟。
const OPEN_EXTERNAL: &str = "openExternalBrowser=1";

/// 在網址上加 `openExternalBrowser=1`（尊重原本的參數與 `#` 之後的片段）。
/// 已經有這個參數就原樣回傳。推廣文裡的連結也建議直接加上它，使用者從 LINE 點開就會直接進預設瀏覽器。
pub fn open_external_url(href: &str) -> String {
    let (base, fragment) = match href.split_once('#') {
        Some((b, f)) => (b, Some(f)),
        None => (href, None),
    };
    let has_param = base
        .split_once('?')
        .is_some_and(|(_, query)| query.split('&').any(|kv| kv.starts_with("openExternalBrowser=")));
    let mut url = base.to_string();
    if !has_param {
        url.push(if base.contains('?') { '&' } else { '?' });
        url.push_str(OPEN_EXTERNAL);
    }
    if let Some(f) = fragment {
        url.push('#');
        url.push_str(f);
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE_IOS: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148 Safari Line/14.8.0";
    const LINE_ANDROID: &str = "Mozilla/5.0 (Linux; Android 14; Pixel 8 Build/AP1A) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/126.0.0.0 Mobile Safari/537.36 Line/14.8.0/LIAPP";
    const SAFARI_IOS: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1";
    const CHROME_ANDROID: &str = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36";

    #[test]
    fn recognises_line_on_iphone_and_android() {
        assert!(is_line_browser(LINE_IOS));
        assert!(is_line_browser(LINE_ANDROID));
    }

    #[test]
    fn default_browsers_are_not_line() {
        assert!(!is_line_browser(SAFARI_IOS));
        assert!(!is_line_browser(CHROME_ANDROID));
        assert!(!is_line_browser(""));
    }

    #[test]
    fn similar_names_are_not_mistaken_for_line() {
        assert!(!is_line_browser("Mozilla/5.0 Deadline/1.0"));
        assert!(!is_line_browser("Mozilla/5.0 Online/2.0"));
        assert!(!is_line_browser("Mozilla/5.0 Linesight/1.0"));
        // 大小寫要對：LINE 寫的是 Line/
        assert!(!is_line_browser("Mozilla/5.0 line/14.8.0"));
    }

    #[test]
    fn adds_the_parameter_to_a_plain_url() {
        assert_eq!(
            open_external_url("https://cornming.github.io/night-yoga/"),
            "https://cornming.github.io/night-yoga/?openExternalBrowser=1"
        );
    }

    #[test]
    fn keeps_existing_query_and_fragment() {
        assert_eq!(
            open_external_url("https://example.com/a?x=1"),
            "https://example.com/a?x=1&openExternalBrowser=1"
        );
        assert_eq!(
            open_external_url("https://example.com/a#top"),
            "https://example.com/a?openExternalBrowser=1#top"
        );
        assert_eq!(
            open_external_url("https://example.com/a?x=1#top"),
            "https://example.com/a?x=1&openExternalBrowser=1#top"
        );
    }

    #[test]
    fn does_not_add_it_twice() {
        let once = "https://example.com/a?openExternalBrowser=1";
        assert_eq!(open_external_url(once), once);
        let mid = "https://example.com/a?x=1&openExternalBrowser=1&y=2#t";
        assert_eq!(open_external_url(mid), mid);
        // 把網址加兩次，結果和加一次一樣
        let base = "https://example.com/a?x=1#t";
        assert_eq!(open_external_url(&open_external_url(base)), open_external_url(base));
    }

    #[test]
    fn a_fragment_that_looks_like_the_parameter_does_not_count() {
        assert_eq!(
            open_external_url("https://example.com/a#?openExternalBrowser=1"),
            "https://example.com/a?openExternalBrowser=1#?openExternalBrowser=1"
        );
    }
}
