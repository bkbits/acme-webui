use chrono::{Duration, Local, NaiveDateTime};

pub const TIME_FMT: &str = "%Y-%m-%d %H:%M:%S";

/// 当前本地时间字符串: YYYY-MM-DD HH:mm:ss
pub fn now_str() -> String {
    Local::now().format(TIME_FMT).to_string()
}

pub fn parse_dt(s: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s, TIME_FMT).ok()
}

pub fn add_days(s: &str, days: i64) -> Option<NaiveDateTime> {
    parse_dt(s).map(|d| d + Duration::days(days))
}

pub fn fmt_naive(d: NaiveDateTime) -> String {
    d.format(TIME_FMT).to_string()
}

/// 按 UTF-8 字符边界安全截断
pub fn trunc(s: &str, n: usize) -> String {
    if s.len() <= n {
        return s.to_string();
    }
    let mut end = n;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n...[内容过长已截断]", &s[..end])
}
