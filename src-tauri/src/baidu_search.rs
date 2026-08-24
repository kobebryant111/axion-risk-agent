//! 百度千帆「百度搜索」：配置、HTTP 调用与吹哨打包检索。

use crate::db;
use crate::llm::crypto;
use crate::models::{BaiduSearchSettings, BaiduSearchSettingsSave, BaiduSearchTestResult, Partner};
use crate::sources::{RawHit, SearchRequest, SourceAdapter, SourceError};
use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

const KEY_ENABLED: &str = "baidu.search.enabled";
const KEY_API_KEY: &str = "baidu.search.api_key_enc";
const KEY_IN_BATCH: &str = "baidu.search.use_in_batch";
/// 跑批轮询游标：名单过长装不下时，下次从哪家继续打包。
const KEY_BATCH_CURSOR: &str = "baidu.search.batch_cursor";
const SEARCH_URL: &str = "https://qianfan.baidubce.com/v2/ai_search/web_search";

/// 问鉴控 / 准入单轮对话：最多 2 次搜索。
pub const MAX_CALLS_PER_OPERATION: u32 = 2;
/// 风险吹哨跑批：一家一次问句（1 家则两次），上限须盖住全量名单。
pub const MAX_CALLS_PER_BATCH: u32 = 250;

/// 百度搜索 query：最多 72 个「字符单位」（汉字计 2）。
const QUERY_UNIT_LIMIT: usize = 72;

/// 单家两次问句，对准网上能搜到的 L1/L2（经营崩盘 vs 监管/消保）。
/// 不塞放大倍数、不良率、工商变更：那些不靠网页检索。
const RISK_ACUTE: &str = "最近有没有爆雷停摆跑路资金池资金链失联挤兑非法集资";
const RISK_REG: &str = "有没有被处罚罚款约谈立案吊销停业投诉维权暴力催收";
/// 2 家及以上：每家一句，不写 OR（千帆不按布尔执行）。
const RISK_COMBINED: &str = "最近有没有爆雷停摆跑路资金池失联处罚投诉立案暴力催收";
/// 本地兜底须覆盖检索锚点；不含「调查/违规/催收」等单独过宽的词。
const SEARCH_RISK_TERMS: &[&str] = &[
    "爆雷", "暴雷", "资金池", "停摆", "跑路", "挤兑", "瘫痪", "失联", "资金链", "非法集资",
    "处罚", "罚款", "约谈", "立案", "吊销", "停业", "投诉", "维权", "暴力催收", "违规催收",
    "失信", "解约", "泄露", "下架", "涉诈",
];

#[derive(Debug, Clone)]
pub struct BaiduSearchConfig {
    pub enabled: bool,
    pub api_key: String,
    /// 是否在风险吹哨跑批中启用（默认 true）
    pub use_in_batch: bool,
}

impl BaiduSearchConfig {
    pub fn ready(&self) -> bool {
        self.enabled && !self.api_key.trim().is_empty()
    }

    pub fn batch_ready(&self) -> bool {
        self.ready() && self.use_in_batch
    }
}

pub fn load_config(conn: &Connection) -> Result<BaiduSearchConfig, String> {
    let enabled = db::get_setting(conn, KEY_ENABLED)?
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let use_in_batch = db::get_setting(conn, KEY_IN_BATCH)?
        .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
        .unwrap_or(true);
    let api_key = match db::get_setting(conn, KEY_API_KEY)? {
        Some(enc) if !enc.is_empty() => crypto::decrypt(&enc).unwrap_or_default(),
        _ => String::new(),
    };
    Ok(BaiduSearchConfig {
        enabled,
        api_key,
        use_in_batch,
    })
}

pub fn to_public(cfg: &BaiduSearchConfig) -> BaiduSearchSettings {
    BaiduSearchSettings {
        enabled: cfg.enabled,
        has_api_key: !cfg.api_key.is_empty(),
        ready: cfg.ready(),
        use_in_batch: cfg.use_in_batch,
    }
}

pub fn save_config(
    conn: &Connection,
    input: &BaiduSearchSettingsSave,
) -> Result<BaiduSearchSettings, String> {
    let current = load_config(conn)?;
    let enabled = input.enabled.unwrap_or(current.enabled);
    let use_in_batch = input.use_in_batch.unwrap_or(current.use_in_batch);

    db::set_setting(conn, KEY_ENABLED, if enabled { "1" } else { "0" })?;
    db::set_setting(conn, KEY_IN_BATCH, if use_in_batch { "1" } else { "0" })?;

    if let Some(key) = &input.api_key {
        let key = key.trim();
        if key.is_empty() {
            db::set_setting(conn, KEY_API_KEY, "")?;
        } else if key != "********" {
            db::set_setting(conn, KEY_API_KEY, &crypto::encrypt(key)?)?;
        }
    }

    Ok(to_public(&load_config(conn)?))
}

#[derive(Debug, Deserialize)]
struct BaiduSearchResponse {
    #[serde(default)]
    references: Vec<SearchHit>,
    #[serde(default)]
    #[allow(dead_code)]
    message: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    code: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct SearchHit {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    snippet: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    date: Option<String>,
}

impl SearchHit {
    fn body(&self) -> String {
        if !self.content.trim().is_empty() {
            self.content.clone()
        } else {
            self.snippet.clone()
        }
    }

    fn published(&self) -> Option<String> {
        self.date.clone()
    }
}

/// 跑批检索窗口。定时用滚动时段；手动指定日期用自然日，并传给百度 page_time。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchWindow {
    LastDay,
    LastWeek,
    /// 闭区间 YYYY-MM-DD（日报起止同一天；周报为该日所在周一至周日）
    Calendar { start: String, end: String },
}

impl SearchWindow {
    pub fn from_job_kind(kind: &str) -> Self {
        if kind.eq_ignore_ascii_case("weekly") {
            Self::LastWeek
        } else {
            Self::LastDay
        }
    }

    pub fn calendar_for(kind: &str, date: NaiveDate) -> Self {
        if kind.eq_ignore_ascii_case("weekly") {
            let (start, end) = iso_week_bounds(date);
            Self::Calendar {
                start: start.to_string(),
                end: end.to_string(),
            }
        } else {
            let d = date.to_string();
            Self::Calendar {
                start: d.clone(),
                end: d,
            }
        }
    }

    pub fn hint(&self) -> String {
        match self {
            Self::LastDay => "最近24小时".into(),
            Self::LastWeek => "最近7天".into(),
            Self::Calendar { start, end } if start == end => start.clone(),
            Self::Calendar { start, end } => format!("{start} ~ {end}"),
        }
    }

    pub fn search_opts(&self, max_results: u32) -> SearchOpts {
        match self {
            Self::LastDay => {
                let today = Local::now().date_naive();
                let yday = today - Duration::days(1);
                SearchOpts {
                    max_results,
                    recency: None,
                    page_time_gte: Some(yday.to_string()),
                    page_time_lte: Some(today.to_string()),
                    drop_before_date: None,
                    drop_after_date: None,
                    strict_undated: false,
                }
            }
            Self::LastWeek => SearchOpts {
                max_results,
                recency: Some("week"),
                page_time_gte: None,
                page_time_lte: None,
                drop_before_date: None,
                drop_after_date: None,
                strict_undated: false,
            },
            Self::Calendar { start, end } => SearchOpts {
                max_results,
                recency: None,
                page_time_gte: Some(start.clone()),
                page_time_lte: Some(end.clone()),
                drop_before_date: Some(start.clone()),
                drop_after_date: Some(end.clone()),
                strict_undated: true,
            },
        }
    }
}

fn iso_week_bounds(date: NaiveDate) -> (NaiveDate, NaiveDate) {
    let days_from_mon = date.weekday().num_days_from_monday() as i64;
    let start = date - Duration::days(days_from_mon);
    let end = start + Duration::days(6);
    debug_assert_eq!(start.weekday(), Weekday::Mon);
    (start, end)
}

/// 百度检索选项。自然日窗口走 `page_time`；本地再用标题/URL 校验。
#[derive(Debug, Clone)]
pub struct SearchOpts {
    pub max_results: u32,
    /// `week` / `month` / `semiyear` / `year`
    pub recency: Option<&'static str>,
    pub page_time_gte: Option<String>,
    pub page_time_lte: Option<String>,
    pub drop_before_date: Option<String>,
    pub drop_after_date: Option<String>,
    pub strict_undated: bool,
}

impl SearchOpts {
    pub fn basic(max_results: u32) -> Self {
        Self {
            max_results,
            recency: None,
            page_time_gte: None,
            page_time_lte: None,
            drop_before_date: None,
            drop_after_date: None,
            strict_undated: false,
        }
    }

    /// 准入瞭望：最近约半年（千帆 page_time + 本地日期门）。
    pub fn last_semiyear(max_results: u32) -> Self {
        let today = Local::now().date_naive();
        let start = today - Duration::days(183);
        Self {
            max_results,
            recency: None,
            page_time_gte: Some(start.to_string()),
            page_time_lte: Some(today.to_string()),
            drop_before_date: Some(start.to_string()),
            drop_after_date: Some(today.to_string()),
            strict_undated: false,
        }
    }
}

fn query_units(s: &str) -> usize {
    s.chars()
        .map(|c| if c.is_ascii() { 1 } else { 2 })
        .sum()
}

fn clip_query(s: &str, max_units: usize) -> String {
    let mut units = 0usize;
    let mut out = String::new();
    for c in s.chars() {
        let w = if c.is_ascii() { 1 } else { 2 };
        if units + w > max_units {
            break;
        }
        units += w;
        out.push(c);
    }
    out
}

/// 底层搜索：返回百度原始 JSON。
pub fn search_raw(
    api_key: &str,
    query: &str,
    opts: &SearchOpts,
) -> Result<serde_json::Value, String> {
    let q = clip_query(query.trim(), QUERY_UNIT_LIMIT);
    if q.is_empty() {
        return Err("搜索词不能为空".into());
    }
    let key = api_key.trim();
    if key.is_empty() {
        return Err("未配置百度千帆搜索 API Key".into());
    }

    let mut body = json!({
        "messages": [{"role": "user", "content": q}],
        "search_source": "baidu_search_v2",
        "resource_type_filter": [{"type": "web", "top_k": opts.max_results.clamp(1, 20)}],
        "sort": { "priority": "auto" },
    });
    if let (Some(gte), Some(lte)) = (&opts.page_time_gte, &opts.page_time_lte) {
        body["search_filter"] = json!({
            "range": { "page_time": { "gte": gte, "lte": lte } }
        });
    } else if let Some(recency) = opts.recency {
        body["search_recency_filter"] = json!(recency);
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .post(SEARCH_URL)
        .header("Authorization", format!("Bearer {key}"))
        .header("X-Appbuilder-Authorization", format!("Bearer {key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("百度搜索网络错误: {e}"))?;

    let status = resp.status();
    let text = resp.text().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("百度搜索 HTTP {status}: {}", truncate(&text, 300)));
    }

    serde_json::from_str(&text)
        .map_err(|e| format!("百度搜索响应解析失败: {e}; body={}", truncate(&text, 200)))
}

pub fn search_hits(
    api_key: &str,
    query: &str,
    max_results: u32,
) -> Result<Vec<RawHit>, String> {
    search_hits_with_opts(api_key, query, &SearchOpts::basic(max_results))
}

pub fn search_hits_with_opts(
    api_key: &str,
    query: &str,
    opts: &SearchOpts,
) -> Result<Vec<RawHit>, String> {
    let value = search_raw(api_key, query, opts)?;
    let parsed: BaiduSearchResponse =
        serde_json::from_value(value).map_err(|e| format!("百度搜索结果映射失败: {e}"))?;
    Ok(parsed
        .references
        .into_iter()
        .filter_map(|r| match classify_hit_date(&r, opts) {
            DateGate::InRange(d) => Some(raw_hit_from_result(r, Some(d))),
            DateGate::Unknown if !opts.strict_undated || has_acute_risk(&r.title, &r.body()) => {
                Some(raw_hit_from_result(r, None))
            }
            _ => None,
        })
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DateHint {
    Full(NaiveDate),
    Md { month: u32, day: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DateGate {
    InRange(NaiveDate),
    OutOfRange,
    Unknown,
}

fn parse_ymd(raw: &str) -> Option<NaiveDate> {
    let day = raw.trim().get(0..10).unwrap_or(raw.trim());
    NaiveDate::parse_from_str(day, "%Y-%m-%d").ok()
}

fn read_uint(chars: &[char], i: usize) -> Option<(u32, usize)> {
    let mut j = i;
    while j < chars.len() && chars[j].is_ascii_digit() {
        j += 1;
    }
    if j == i {
        return None;
    }
    let s: String = chars[i..j].iter().collect();
    Some((s.parse().ok()?, j))
}

fn try_parse_iso_at(chars: &[char], i: usize) -> Option<(NaiveDate, usize)> {
    if i + 10 > chars.len() {
        return None;
    }
    let s: String = chars[i..i + 10].iter().collect();
    NaiveDate::parse_from_str(&s, "%Y-%m-%d")
        .ok()
        .or_else(|| NaiveDate::parse_from_str(&s, "%Y/%m/%d").ok())
        .map(|d| (d, 10))
}

fn try_parse_cn_at(chars: &[char], i: usize) -> Option<(DateHint, usize)> {
    let mut j = i;
    let mut year = None;
    if j + 5 <= chars.len()
        && chars[j].is_ascii_digit()
        && chars[j + 1].is_ascii_digit()
        && chars[j + 2].is_ascii_digit()
        && chars[j + 3].is_ascii_digit()
        && chars[j + 4] == '年'
    {
        let y: String = chars[j..j + 4].iter().collect();
        year = y.parse().ok();
        j += 5;
    }
    let (month, after_m) = read_uint(chars, j)?;
    if after_m >= chars.len() || chars[after_m] != '月' {
        return None;
    }
    let (day, after_d) = read_uint(chars, after_m + 1)?;
    if after_d >= chars.len() || chars[after_d] != '日' {
        return None;
    }
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let consumed = after_d + 1 - i;
    if let Some(y) = year {
        let d = NaiveDate::from_ymd_opt(y, month, day)?;
        Some((DateHint::Full(d), consumed))
    } else {
        Some((DateHint::Md { month, day }, consumed))
    }
}

fn extract_date_hints(text: &str) -> Vec<DateHint> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if let Some((date, n)) = try_parse_iso_at(&chars, i) {
            out.push(DateHint::Full(date));
            i += n;
            continue;
        }
        if let Some((date, n)) = try_parse_dmy_cn_at(&chars, i) {
            out.push(DateHint::Full(date));
            i += n;
            continue;
        }
        if let Some((hint, n)) = try_parse_cn_at(&chars, i) {
            out.push(hint);
            i += n;
            continue;
        }
        i += 1;
    }
    out
}

fn skip_ws(chars: &[char], mut i: usize) -> usize {
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    i
}

/// 「26 6月 2026」「26日6月2026」这类西式/混排日期。
fn try_parse_dmy_cn_at(chars: &[char], i: usize) -> Option<(NaiveDate, usize)> {
    let (day, j) = read_uint(chars, i)?;
    let mut j = skip_ws(chars, j);
    if j < chars.len() && chars[j] == '日' {
        j = skip_ws(chars, j + 1);
    }
    let (month, k) = read_uint(chars, j)?;
    if k >= chars.len() || chars[k] != '月' {
        return None;
    }
    let k = skip_ws(chars, k + 1);
    let (year, end) = read_uint(chars, k)?;
    if !(2000..=2100).contains(&year) || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let d = NaiveDate::from_ymd_opt(year as i32, month, day)?;
    Some((d, end - i))
}

fn in_window(d: NaiveDate, start: NaiveDate, end: NaiveDate) -> bool {
    d >= start && d <= end
}

/// 线索日期用「发布时间」：百度 `date` > URL 稿号 > 正文里的日期（正文常是事发日，会把 6.27 发的稿标成 6.24）。
fn classify_hit_date(r: &SearchHit, opts: &SearchOpts) -> DateGate {
    let start = opts.drop_before_date.as_deref().and_then(parse_ymd);
    let end = opts.drop_after_date.as_deref().and_then(parse_ymd);
    let url_dates = extract_url_dates(&r.url);
    let mut body_full = Vec::new();
    let mut mds = Vec::new();
    let blob = format!("{} {}", r.title, r.body());
    for hint in extract_date_hints(&blob) {
        match hint {
            DateHint::Full(d) => body_full.push(d),
            DateHint::Md { month, day } => mds.push((month, day)),
        }
    }
    let published = r.published().as_deref().and_then(parse_ymd);

    let Some(start) = start else {
        return published
            .or(url_dates.first().copied())
            .or(body_full.first().copied())
            .map(DateGate::InRange)
            .unwrap_or(DateGate::Unknown);
    };
    let end = end.unwrap_or(start);

    if let Some(d) = published {
        return if in_window(d, start, end) {
            DateGate::InRange(d)
        } else {
            DateGate::OutOfRange
        };
    }
    if !url_dates.is_empty() {
        if let Some(d) = url_dates.into_iter().find(|d| in_window(*d, start, end)) {
            return DateGate::InRange(d);
        }
        return DateGate::OutOfRange;
    }
    if !body_full.is_empty() {
        if let Some(d) = body_full.into_iter().find(|d| in_window(*d, start, end)) {
            return DateGate::InRange(d);
        }
        return DateGate::OutOfRange;
    }
    if !mds.is_empty() {
        for (month, day) in mds {
            for y in start.year()..=end.year() {
                if let Some(d) = NaiveDate::from_ymd_opt(y, month, day) {
                    if in_window(d, start, end) {
                        return DateGate::InRange(d);
                    }
                }
            }
        }
        return DateGate::OutOfRange;
    }
    DateGate::Unknown
}

fn raw_hit_from_result(r: SearchHit, event_date: Option<NaiveDate>) -> RawHit {
    let content = r.body();
    let event_time = event_date
        .map(|d| d.to_string())
        .or_else(|| r.published().as_deref().and_then(parse_ymd).map(|d| d.to_string()))
        .unwrap_or_else(|| "日期不详".into());
    let summary = if content.chars().count() > 180 {
        format!("{}…", content.chars().take(180).collect::<String>())
    } else {
        content.clone()
    };
    let credibility = guess_credibility(&r.url);
    let title = if r.title.is_empty() {
        r.url.clone()
    } else {
        r.title
    };
    RawHit {
        source_id: "baidu".into(),
        title,
        summary,
        url: r.url,
        event_time,
        body: content,
        credibility,
        related_party_term: None,
        heimao: None,
    }
}

fn guess_credibility(url: &str) -> String {
    let u = url.to_lowercase();
    if u.contains(".gov.cn")
        || u.contains("court.gov")
        || u.contains("pbc.gov")
        || u.contains("cbirc")
        || u.contains("nfra.gov")
        || u.contains("csrc.gov")
    {
        "A".into()
    } else if u.contains("reuters")
        || u.contains("bloomberg")
        || u.contains("caixin")
        || u.contains("财新")
        || u.contains("xinhua")
        || u.contains("people.com.cn")
        || u.contains("thepaper")
        || u.contains("36kr")
        || u.contains("cls.cn")
        || u.contains("yicai")
        || u.contains("stcn.com")
    {
        "B".into()
    } else {
        "C".into()
    }
}

#[allow(dead_code)]
pub fn build_partner_query(req: &SearchRequest) -> String {
    focused_query(&best_brand(&req.partner), RISK_COMBINED)
}

fn looks_like_uscc(s: &str) -> bool {
    let n = s.chars().count();
    n >= 16 && s.chars().all(|c| c.is_ascii_alphanumeric())
}

fn is_short_search_term(s: &str) -> bool {
    let n = s.chars().count();
    n >= 3 && n <= 16 && !looks_like_uscc(s)
}

/// 单家：全称 + 简称 + 若干短名（新闻常用）。多家打包：每家只塞一个短名。
fn search_terms_for_query(p: &Partner, single: bool) -> Vec<String> {
    let expanded = crate::partners::expand_watch_terms(&p.name, &p.alias, &p.lexicon);
    if single {
        let mut terms = Vec::new();
        let full = p.name.trim();
        if !full.is_empty() {
            terms.push(full.to_string());
        }
        let alias = p.alias.trim();
        if !alias.is_empty() && alias != full {
            terms.push(alias.to_string());
        }
        let mut shorts: Vec<String> = expanded
            .into_iter()
            .filter(|t| is_short_search_term(t) && t != full && t != alias)
            .collect();
        shorts.sort_by_key(|s| s.chars().count());
        for s in shorts {
            if terms.len() >= 5 {
                break;
            }
            if !terms.iter().any(|x| x == &s) {
                terms.push(s);
            }
        }
        terms
    } else {
        let alias = p.alias.trim();
        if is_short_search_term(alias) {
            return vec![alias.to_string()];
        }
        expanded
            .into_iter()
            .filter(|t| is_short_search_term(t))
            .min_by_key(|s| s.chars().count())
            .map(|s| vec![s])
            .unwrap_or_else(|| {
                if full_or_name(p).is_empty() {
                    Vec::new()
                } else {
                    vec![full_or_name(p)]
                }
            })
    }
}

fn full_or_name(p: &Partner) -> String {
    let n = p.name.trim();
    if n.is_empty() {
        p.alias.trim().to_string()
    } else {
        n.to_string()
    }
}

fn best_brand(p: &Partner) -> String {
    let mut shorts: Vec<String> = search_terms_for_query(p, true)
        .into_iter()
        .filter(|t| is_short_search_term(t))
        .collect();
    shorts.sort_by_key(|s| s.chars().count());
    shorts
        .iter()
        .find(|s| s.contains("数科") && s.chars().count() >= 4)
        .cloned()
        .or_else(|| shorts.into_iter().find(|s| s.chars().count() >= 4))
        .or_else(|| {
            let a = p.alias.trim();
            if !a.is_empty() {
                Some(a.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| full_or_name(p))
}

fn focused_query(brand: &str, risk: &str) -> String {
    clip_query(&format!("\"{brand}\" {risk}"), QUERY_UNIT_LIMIT)
}

/// 准入等场景：从公司全称压出检索短名（与吹哨 best_brand 同一套规则）。
pub fn search_brand_from_name(name: &str) -> String {
    let name = name.trim();
    let lexicon = crate::partners::expand_watch_terms(name, "", &[]);
    let p = Partner {
        id: String::new(),
        name: name.into(),
        alias: String::new(),
        group_name: String::new(),
        uscc: String::new(),
        partner_type: String::new(),
        partner_type_label: String::new(),
        related_parties: vec![],
        status: String::new(),
        lexicon,
    };
    best_brand(&p)
}

/// 准入瞭望全网搜：最多两句、不写 OR。
/// 第一句对齐风险吹哨打包问法；第二句补工商/诉讼（千帆按自然语言检索）。
pub fn admission_web_queries(brand: &str) -> Vec<String> {
    vec![
        focused_query(brand, RISK_COMBINED),
        focused_query(brand, "最近有没有工商变更诉讼被执行"),
    ]
}

/// 标题里的宣传稿套话。摘要常被塞进检索词，不能只靠正文判断。
const PR_TITLE_MARKS: &[&str] = &[
    "成长之路",
    "向阳而生",
    "新征程",
    "政企同频",
    "紧跟监管",
    "合规新征程",
    "扎根营口",
    "发展密码",
    "探寻其",
];
/// 标题专有：正文里太常见、标题出现则更像风险稿。
const TITLE_RISK_EXTRA: &[&str] = &["催收", "征信", "逾期", "截留", "停服", "误伤"];

fn is_promotional_title(title: &str) -> bool {
    PR_TITLE_MARKS.iter().any(|k| title.contains(k))
}

fn title_has_risk(title: &str) -> bool {
    SEARCH_RISK_TERMS.iter().any(|k| title.contains(k))
        || TITLE_RISK_EXTRA.iter().any(|k| title.contains(k))
}

/// 是否留下这条网页：宣传标题直接丢；标题无风险词也丢（防摘要被检索词污染）。
pub fn keep_search_hit(title: &str, body: &str) -> bool {
    let _ = body;
    if is_promotional_title(title) {
        return false;
    }
    title_has_risk(title)
}

fn has_acute_risk(title: &str, content: &str) -> bool {
    const KEYS: &[&str] = &["爆雷", "暴雷", "资金池", "停摆", "跑路", "挤兑"];
    KEYS.iter().any(|k| title.contains(k) || content.contains(k))
}

fn extract_url_dates(url: &str) -> Vec<NaiveDate> {
    let mut out = Vec::new();
    for hint in extract_date_hints(url) {
        if let DateHint::Full(d) = hint {
            out.push(d);
        }
    }
    let chars: Vec<char> = url.chars().collect();
    let mut i = 0;
    while i + 8 <= chars.len() {
        if let Some(date) = try_parse_ymd8_at(&chars, i) {
            out.push(date);
            i += 8;
            continue;
        }
        i += 1;
    }
    out
}

/// 路径稿号 `20260702235026…`：取前 8 位 YYYYMMDD，允许后面继续跟数字。
fn try_parse_ymd8_at(chars: &[char], i: usize) -> Option<NaiveDate> {
    if i + 8 > chars.len() {
        return None;
    }
    if i > 0 && chars[i - 1].is_ascii_digit() {
        return None;
    }
    if !chars[i..i + 8].iter().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let s: String = chars[i..i + 8].iter().collect();
    let y: i32 = s[0..4].parse().ok()?;
    let m: u32 = s[4..6].parse().ok()?;
    let d: u32 = s[6..8].parse().ok()?;
    if !(2020..=2035).contains(&y) {
        return None;
    }
    NaiveDate::from_ymd_opt(y, m, d)
}

pub fn test_connection(cfg: &BaiduSearchConfig) -> BaiduSearchTestResult {
    if !cfg.ready() {
        return BaiduSearchTestResult {
            ok: false,
            message: "请先启用并填写百度千帆搜索 API Key".into(),
            result_count: 0,
        };
    }
    match search_raw(&cfg.api_key, "消费金融 监管", &SearchOpts::basic(1)) {
        Ok(v) => {
            let n = v
                .get("references")
                .and_then(|r| r.as_array())
                .map(|a| a.len() as u32)
                .unwrap_or(0);
            BaiduSearchTestResult {
                ok: true,
                message: format!("百度搜索连通正常，返回 {n} 条结果"),
                result_count: n,
            }
        }
        Err(e) => BaiduSearchTestResult {
            ok: false,
            message: e,
            result_count: 0,
        },
    }
}

#[allow(dead_code)]
pub struct BaiduSearchAdapter {
    pub api_key: String,
    pub max_results: u32,
}

#[allow(dead_code)]
impl SourceAdapter for BaiduSearchAdapter {
    fn source_id(&self) -> &'static str {
        "baidu"
    }

    fn search(&self, req: &SearchRequest) -> Result<Vec<RawHit>, SourceError> {
        if self.api_key.trim().is_empty() {
            return Err(SourceError::Blocked("百度搜索 API Key 未配置".into()));
        }
        let query = build_partner_query(req);
        search_hits_with_opts(
            &self.api_key,
            &query,
            &SearchWindow::LastDay.search_opts(self.max_results.min(5)),
        )
        .map_err(SourceError::Blocked)
    }
}

#[derive(Debug, Default)]
pub struct BundledSearchOutcome {
    pub calls: u32,
    /// partner_id → 命中该机构词库的检索结果
    pub hits_by_partner: HashMap<String, Vec<RawHit>>,
    pub errors: Vec<String>,
    pub partners_covered: u32,
    pub queries: Vec<String>,
}

/// 跑批：1 家两句问句；2 家及以上每家一句，不用 OR。
pub fn search_partners_bundled(
    conn: &Connection,
    api_key: &str,
    partners: &[Partner],
    max_results: u32,
    persist_cursor: bool,
    window: &SearchWindow,
) -> Result<BundledSearchOutcome, String> {
    let mut out = BundledSearchOutcome::default();
    if partners.is_empty() {
        return Ok(out);
    }
    let n = partners.len();
    let cursor = if persist_cursor {
        db::get_setting(conn, KEY_BATCH_CURSOR)?
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0)
            % n
    } else {
        0
    };

    let jobs = plan_search_jobs(partners, cursor, window, max_results.max(8));
    out.partners_covered = jobs.covered;

    for job in &jobs.items {
        out.queries.push(job.query.clone());
        out.calls += 1;
        match search_hits_with_opts(api_key, &job.query, &job.opts) {
            Ok(hits) => {
                for hit in hits {
                    let text = format!("{} {} {}", hit.title, hit.summary, hit.body);
                    if !keep_search_hit(&hit.title, &text) {
                        continue;
                    }
                    for p in &job.partners {
                        let terms = crate::partners::expand_watch_terms(
                            &p.name,
                            &p.alias,
                            &p.lexicon,
                        );
                        let matched = terms.iter().any(|t| {
                            let t = t.trim();
                            t.chars().count() >= 2 && text.contains(t)
                        });
                        if matched {
                            let list = out.hits_by_partner.entry(p.id.clone()).or_default();
                            if !list.iter().any(|h| h.url == hit.url) {
                                list.push(hit.clone());
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let names: Vec<&str> = job.partners.iter().map(|p| p.name.as_str()).collect();
                out.errors
                    .push(format!("baidu @ [{}]: {e}", names.join("、")));
            }
        }
    }

    if persist_cursor {
        let next = if jobs.covered as usize >= n {
            0
        } else {
            (cursor + jobs.covered as usize) % n.max(1)
        };
        db::set_setting(conn, KEY_BATCH_CURSOR, &next.to_string())?;
    }
    Ok(out)
}

struct SearchJob<'a> {
    query: String,
    opts: SearchOpts,
    partners: Vec<&'a Partner>,
}

struct SearchPlan<'a> {
    items: Vec<SearchJob<'a>>,
    covered: u32,
}

fn plan_search_jobs<'a>(
    partners: &'a [Partner],
    cursor: usize,
    window: &SearchWindow,
    max_results: u32,
) -> SearchPlan<'a> {
    if partners.is_empty() {
        return SearchPlan {
            items: Vec::new(),
            covered: 0,
        };
    }
    if partners.len() == 1 {
        let p = &partners[0];
        let brand = best_brand(p);
        let opts = window.search_opts(max_results);
        return SearchPlan {
            items: vec![
                SearchJob {
                    query: focused_query(&brand, RISK_ACUTE),
                    opts: opts.clone(),
                    partners: vec![p],
                },
                SearchJob {
                    query: focused_query(&brand, RISK_REG),
                    opts,
                    partners: vec![p],
                },
            ],
            covered: 1,
        };
    }

    let n = partners.len();
    let start = cursor % n;
    let opts = window.search_opts(max_results.min(8));
    let cap = MAX_CALLS_PER_BATCH as usize;
    let mut items = Vec::new();
    let mut covered = 0usize;
    while covered < n && items.len() < cap {
        let p = &partners[(start + covered) % n];
        let brand = best_brand(p);
        items.push(SearchJob {
            query: focused_query(&brand, RISK_COMBINED),
            opts: opts.clone(),
            partners: vec![p],
        });
        covered += 1;
    }
    SearchPlan {
        items,
        covered: covered as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_partner(id: &str, name: &str, alias: &str) -> Partner {
        Partner {
            id: id.into(),
            name: name.into(),
            alias: alias.into(),
            group_name: String::new(),
            uscc: String::new(),
            partner_type: "loan".into(),
            partner_type_label: "助贷机构".into(),
            related_parties: vec![],
            status: "active".into(),
            lexicon: vec![name.into(), alias.into()]
                .into_iter()
                .filter(|s: &String| !s.is_empty())
                .collect(),
        }
    }

    fn sample(title: &str, url: &str, content: &str, date: Option<&str>) -> SearchHit {
        SearchHit {
            title: title.into(),
            url: url.into(),
            snippet: content.into(),
            content: content.into(),
            date: date.map(str::to_string),
        }
    }

    #[test]
    fn last_day_uses_page_time_window() {
        let opts = SearchWindow::LastDay.search_opts(5);
        assert!(opts.page_time_gte.is_some());
        assert!(opts.page_time_lte.is_some());
        assert!(opts.drop_before_date.is_none());
        let week = SearchWindow::LastWeek.search_opts(5);
        assert_eq!(week.recency, Some("week"));
        assert!(week.drop_before_date.is_none());
    }

    #[test]
    fn calendar_day_sends_page_time() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
        let opts = SearchWindow::calendar_for("daily", date).search_opts(5);
        assert!(opts.recency.is_none());
        assert_eq!(opts.page_time_gte.as_deref(), Some("2026-08-12"));
        assert_eq!(opts.page_time_lte.as_deref(), Some("2026-08-12"));
        assert_eq!(opts.drop_before_date.as_deref(), Some("2026-08-12"));
        assert!(opts.strict_undated);
        let week = SearchWindow::calendar_for("weekly", date).search_opts(5);
        assert_eq!(week.page_time_gte.as_deref(), Some("2026-08-10"));
        assert_eq!(week.drop_after_date.as_deref(), Some("2026-08-16"));
    }

    #[test]
    fn admission_uses_combined_risk_query_and_half_year_window() {
        let brand = search_brand_from_name("天津东华融资担保有限公司");
        assert!(brand.contains("融担") || brand.contains("东华"), "{brand}");
        let qs = admission_web_queries(&brand);
        assert_eq!(qs.len(), 2);
        assert!(
            qs[0].contains("爆雷") && qs[0].contains("处罚") && qs[0].contains("暴力催收"),
            "{}",
            qs[0]
        );
        assert!(qs[1].contains("工商") && qs[1].contains("诉讼"), "{}", qs[1]);
        assert!(qs.iter().all(|q| !q.contains(" OR ")));
        assert!(qs.iter().all(|q| query_units(q) <= QUERY_UNIT_LIMIT));
        let opts = SearchOpts::last_semiyear(8);
        let today = Local::now().date_naive().to_string();
        assert_eq!(opts.page_time_lte.as_deref(), Some(today.as_str()));
        let start = (Local::now().date_naive() - Duration::days(183)).to_string();
        assert_eq!(opts.page_time_gte.as_deref(), Some(start.as_str()));
    }

    #[test]
    fn drops_stale_published_dates_for_today_filter() {
        let june = SearchWindow::calendar_for(
            "weekly",
            NaiveDate::from_ymd_opt(2026, 6, 22).unwrap(),
        )
        .search_opts(5);
        let old = sample("a", "u", "c", Some("2020-01-01"));
        let in_week = sample("b", "u", "c", Some("2026-06-24T08:00:00Z"));
        let unknown = sample("c", "u", "c", None);
        assert_eq!(classify_hit_date(&old, &june), DateGate::OutOfRange);
        assert_eq!(
            classify_hit_date(&in_week, &june),
            DateGate::InRange(NaiveDate::from_ymd_opt(2026, 6, 24).unwrap())
        );
        assert_eq!(classify_hit_date(&unknown, &june), DateGate::Unknown);
        let later = sample("d", "u", "c", Some("2026-08-05"));
        assert_eq!(classify_hit_date(&later, &june), DateGate::OutOfRange);
    }

    #[test]
    fn drops_2024_aug_news_from_june_2026_week() {
        let june = SearchWindow::calendar_for(
            "weekly",
            NaiveDate::from_ymd_opt(2026, 6, 22).unwrap(),
        )
        .search_opts(5);
        assert_eq!(june.drop_before_date.as_deref(), Some("2026-06-22"));
        assert_eq!(june.drop_after_date.as_deref(), Some("2026-06-28"));
        let jrj = sample(
            "辽宁自贸试验区（营口片区）桔子数字科技有限公司8月19日被投诉，涉及消费金额2195.00元",
            "https://www.jrj.com.cn/x",
            "2024-08-19 11:20 发布于：北京市",
            None,
        );
        assert_eq!(classify_hit_date(&jrj, &june), DateGate::OutOfRange);
        let title_only = sample("某公司8月19日被投诉", "u", "", None);
        assert_eq!(classify_hit_date(&title_only, &june), DateGate::OutOfRange);
        let june_hit = sample("某公司6月25日被处罚", "u", "", None);
        assert_eq!(
            classify_hit_date(&june_hit, &june),
            DateGate::InRange(NaiveDate::from_ymd_opt(2026, 6, 25).unwrap())
        );
        let dmy = sample(
            "700亿助贷平台桔子数科惊天一雷",
            "u",
            "26 6月 2026 · 7 MIN READ 消费金融",
            None,
        );
        assert_eq!(
            classify_hit_date(&dmy, &june),
            DateGate::InRange(NaiveDate::from_ymd_opt(2026, 6, 26).unwrap())
        );
    }

    #[test]
    fn single_partner_uses_short_natural_queries() {
        let p = demo_partner(
            "juzi",
            "辽宁自贸试验区（营口片区）桔子数字科技有限公司",
            "桔子科技",
        );
        let window = SearchWindow::calendar_for(
            "weekly",
            NaiveDate::from_ymd_opt(2026, 7, 2).unwrap(),
        );
        let plan = plan_search_jobs(std::slice::from_ref(&p), 0, &window, 10);
        assert_eq!(plan.items.len(), 2);
        let q0 = &plan.items[0].query;
        assert!(q0.contains("桔子数科"), "{q0}");
        assert!(q0.contains("爆雷") && q0.contains("停摆") && q0.contains("跑路"), "{q0}");
        assert!(q0.contains("资金池") && q0.contains("非法集资"), "{q0}");
        assert!(query_units(q0) <= QUERY_UNIT_LIMIT, "{q0}");
        assert!(!q0.contains(" OR "), "{q0}");
        let q1 = &plan.items[1].query;
        assert!(q1.contains("处罚") && q1.contains("立案") && q1.contains("投诉"), "{q1}");
        assert!(q1.contains("暴力催收"), "{q1}");
        assert!(query_units(q1) <= QUERY_UNIT_LIMIT, "{q1}");
        assert!(!q1.contains(" OR "), "{q1}");
        assert!(query_units(&focused_query("宁银消费金融", RISK_ACUTE)) <= QUERY_UNIT_LIMIT);
        assert!(query_units(&focused_query("宁银消费金融", RISK_REG)) <= QUERY_UNIT_LIMIT);
        assert!(!q0.contains("辽宁自贸试验区（营口片区）"), "{q0}");
        assert!(query_units(q0) <= QUERY_UNIT_LIMIT);
        assert_eq!(
            plan.items[0].opts.page_time_gte.as_deref(),
            Some("2026-06-29")
        );
    }

    #[test]
    fn keeps_undated_baolei_headline() {
        let week = SearchWindow::calendar_for(
            "weekly",
            NaiveDate::from_ymd_opt(2026, 7, 2).unwrap(),
        )
        .search_opts(5);
        let h = sample(
            "从校园贷到资金池！桔子数科爆雷全解析",
            "https://www.163.com/dy/article/XXXX.html",
            "桔子数科资金链紧张",
            None,
        );
        assert_eq!(classify_hit_date(&h, &week), DateGate::Unknown);
        assert!(has_acute_risk(&h.title, &h.body()));
    }

    #[test]
    fn drops_pr_piece_without_risk_terms() {
        assert!(!keep_search_hit(
            "扎根营口，向阳而生：桔子数科的政企同频成长之路",
            "凭借扎实的技术积累与稳健的合规运营，成功跻身33家银行及消费金融公司的白名单 爆雷 停摆 处罚"
        ));
        assert!(!keep_search_hit(
            "桔子数科紧跟监管步伐，开启助贷合规新征程？",
            "桔子数科表示将严格落实监管要求"
        ));
        assert!(keep_search_hit(
            "钱还了，征信还逾期，咋回事？",
            "桔小花宜口袋突然失联"
        ));
        assert!(keep_search_hit(
            "从校园贷到资金池！桔子数科爆雷全解析",
            ""
        ));
    }

    #[test]
    fn reads_compact_yyyymmdd_from_eastmoney_url() {
        let july_week = SearchWindow::calendar_for(
            "weekly",
            NaiveDate::from_ymd_opt(2026, 7, 2).unwrap(),
        )
        .search_opts(5);
        let june_week = SearchWindow::calendar_for(
            "weekly",
            NaiveDate::from_ymd_opt(2026, 6, 22).unwrap(),
        )
        .search_opts(5);
        let h = sample(
            "桔子数科停服：数万用户的钱去哪了？",
            "https://caifuhao.eastmoney.com/news/20260702235026218845550",
            "外参财观",
            None,
        );
        assert_eq!(
            classify_hit_date(&h, &july_week),
            DateGate::InRange(NaiveDate::from_ymd_opt(2026, 7, 2).unwrap())
        );
        assert_eq!(classify_hit_date(&h, &june_week), DateGate::OutOfRange);
    }

    #[test]
    fn prefers_publish_date_over_incident_date_in_body() {
        let week = SearchWindow::calendar_for(
            "weekly",
            NaiveDate::from_ymd_opt(2026, 6, 22).unwrap(),
        )
        .search_opts(5);
        let h = sample(
            "钱还了，征信还逾期，咋回事？",
            "https://example.com/x",
            "2026年6月24日晚，营口这座海边小城的几个借贷软件突然闭口",
            Some("2026-06-27 19:24"),
        );
        assert_eq!(
            classify_hit_date(&h, &week),
            DateGate::InRange(NaiveDate::from_ymd_opt(2026, 6, 27).unwrap())
        );
    }

    #[test]
    fn five_partners_each_get_one_query_without_or() {
        let partners: Vec<Partner> = (0..5)
            .map(|i| {
                demo_partner(
                    &format!("p{i}"),
                    &format!("测试机构{i:02}有限公司"),
                    &format!("测企{i:02}"),
                )
            })
            .collect();
        let window = SearchWindow::calendar_for(
            "weekly",
            NaiveDate::from_ymd_opt(2026, 6, 22).unwrap(),
        );
        let plan = plan_search_jobs(&partners, 0, &window, 8);
        assert_eq!(plan.items.len(), 5);
        assert_eq!(plan.covered, 5);
        for (i, job) in plan.items.iter().enumerate() {
            assert_eq!(job.partners.len(), 1);
            assert!(!job.query.contains(" OR "), "{}", job.query);
            assert!(job.query.contains("爆雷"), "{}", job.query);
            assert!(job.query.contains("处罚"), "{}", job.query);
            assert!(query_units(&job.query) <= QUERY_UNIT_LIMIT, "{}", job.query);
            let brand = format!("测企{i:02}");
            assert!(
                job.query.contains(&brand) || job.partners[0].alias.contains(&brand),
                "{}",
                job.query
            );
        }
    }

    #[test]
    fn twelve_partners_one_query_each() {
        let partners: Vec<Partner> = (0..12)
            .map(|i| demo_partner(&format!("p{i}"), &format!("测试机构{i:02}有限公司"), &format!("测企{i:02}")))
            .collect();
        let window = SearchWindow::calendar_for(
            "weekly",
            NaiveDate::from_ymd_opt(2026, 6, 22).unwrap(),
        );
        let plan = plan_search_jobs(&partners, 0, &window, 8);
        assert_eq!(plan.covered, 12);
        assert_eq!(plan.items.len(), 12);
        assert!(plan.items.iter().all(|j| !j.query.contains(" OR ")));
    }
}


fn truncate(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        format!("{t}…")
    } else {
        t
    }
}
