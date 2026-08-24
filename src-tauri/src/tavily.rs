//! Tavily 全网搜索：配置、HTTP 调用与吹哨 SourceAdapter。

use crate::db;
use crate::llm::crypto;
use crate::models::{Partner, TavilySettings, TavilySettingsSave, TavilyTestResult};
use crate::sources::{RawHit, SearchRequest, SourceAdapter, SourceError};
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

const KEY_ENABLED: &str = "tavily.enabled";
const KEY_API_KEY: &str = "tavily.api_key_enc";
const KEY_IN_BATCH: &str = "tavily.use_in_batch";
/// 跑批轮询游标：名单过长装不下时，下次从哪家继续打包。
const KEY_BATCH_CURSOR: &str = "tavily.batch_cursor";
const SEARCH_URL: &str = "https://api.tavily.com/search";

/// 单次业务操作（一次跑批 / 问鉴控一轮对话）允许的 Tavily HTTP 调用上限。
/// 「3 次以下」= 最多 2 次，按次数计费时强制节流。
pub const MAX_CALLS_PER_OPERATION: u32 = 2;

/// 单条 query 中机构名 OR 串的大致字符预算（避免检索词过长被截断）。
const QUERY_NAME_BUDGET: usize = 320;

const RISK_TAIL: &str =
    "(处罚 OR 投诉 OR 违规 OR 失信 OR 诉讼 OR 监管 OR 风险 OR 罚款 OR 催收)";

#[derive(Debug, Clone)]
pub struct TavilyConfig {
    pub enabled: bool,
    pub api_key: String,
    /// 是否在风险吹哨跑批中启用（默认 true）
    pub use_in_batch: bool,
}

impl TavilyConfig {
    pub fn ready(&self) -> bool {
        self.enabled && !self.api_key.trim().is_empty()
    }

    pub fn batch_ready(&self) -> bool {
        self.ready() && self.use_in_batch
    }
}

pub fn load_config(conn: &Connection) -> Result<TavilyConfig, String> {
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
    Ok(TavilyConfig {
        enabled,
        api_key,
        use_in_batch,
    })
}

pub fn to_public(cfg: &TavilyConfig) -> TavilySettings {
    TavilySettings {
        enabled: cfg.enabled,
        has_api_key: !cfg.api_key.is_empty(),
        ready: cfg.ready(),
        use_in_batch: cfg.use_in_batch,
    }
}

pub fn save_config(conn: &Connection, input: &TavilySettingsSave) -> Result<TavilySettings, String> {
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
struct TavilySearchResponse {
    #[serde(default)]
    results: Vec<TavilyResult>,
}

#[derive(Debug, Deserialize)]
struct TavilyResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    published_date: Option<String>,
}

/// 跑批检索窗口：滚动时段，不是自然日 0 点切分。
/// 日报 = 最近约 24 小时（早上 8 点跑则覆盖昨 8 点～今 8 点）；周报 = 最近约 7 天。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchWindow {
    LastDay,
    LastWeek,
}

impl SearchWindow {
    pub fn from_job_kind(kind: &str) -> Self {
        if kind.eq_ignore_ascii_case("weekly") {
            Self::LastWeek
        } else {
            Self::LastDay
        }
    }

    pub fn as_tavily(self) -> &'static str {
        match self {
            Self::LastDay => "day",
            Self::LastWeek => "week",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::LastDay => "最近24小时",
            Self::LastWeek => "最近7天",
        }
    }

    pub fn search_opts(self, max_results: u32) -> SearchOpts {
        SearchOpts {
            max_results,
            search_depth: "basic",
            time_range: Some(self.as_tavily()),
            start_date: None,
            drop_before_date: None,
        }
    }
}

/// Tavily 检索选项。跑批用 [`SearchWindow`]（`time_range=day|week`），不要叠 `start_date`。
#[derive(Debug, Clone)]
pub struct SearchOpts {
    pub max_results: u32,
    pub search_depth: &'static str,
    /// `day` / `week` / `month` / `year`
    pub time_range: Option<&'static str>,
    /// YYYY-MM-DD，结果发布不早于该日
    pub start_date: Option<String>,
    /// 本地再滤：丢弃能解析且明显早于该日的 published_date（无日期则保留）
    pub drop_before_date: Option<String>,
}

impl SearchOpts {
    pub fn basic(max_results: u32) -> Self {
        Self {
            max_results,
            search_depth: "basic",
            time_range: None,
            start_date: None,
            drop_before_date: None,
        }
    }
}

/// 底层搜索：返回原始结果 JSON 友好结构。
pub fn search_raw(
    api_key: &str,
    query: &str,
    opts: &SearchOpts,
) -> Result<serde_json::Value, String> {
    let q = query.trim();
    if q.is_empty() {
        return Err("搜索词不能为空".into());
    }
    let key = api_key.trim();
    if key.is_empty() {
        return Err("未配置 Tavily API Key".into());
    }

    // 始终 basic：advanced 按 Tavily 计费通常更贵；单次 HTTP = 1 次计费。
    let depth = if opts.search_depth == "advanced" {
        "basic"
    } else {
        opts.search_depth
    };
    let mut body = json!({
        "query": q,
        "search_depth": depth,
        "max_results": opts.max_results.clamp(1, 5),
        "include_answer": false,
        "include_raw_content": false,
    });
    if let Some(tr) = opts.time_range {
        body["time_range"] = json!(tr);
    }
    if let Some(sd) = &opts.start_date {
        body["start_date"] = json!(sd);
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .post(SEARCH_URL)
        .header("Authorization", format!("Bearer {key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("Tavily 网络错误: {e}"))?;

    let status = resp.status();
    let text = resp.text().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Tavily HTTP {status}: {}", truncate(&text, 300)));
    }

    serde_json::from_str(&text).map_err(|e| format!("Tavily 响应解析失败: {e}; body={}", truncate(&text, 200)))
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
    let parsed: TavilySearchResponse =
        serde_json::from_value(value).map_err(|e| format!("Tavily 结果映射失败: {e}"))?;
    Ok(parsed
        .results
        .into_iter()
        .filter(|r| keep_by_published_date(r, opts.drop_before_date.as_deref()))
        .map(|r| raw_hit_from_result(r))
        .collect())
}

/// 有明确发布日且早于 cutoff 则丢弃；无日期或解析失败则保留（避免误杀）。
fn keep_by_published_date(r: &TavilyResult, cutoff: Option<&str>) -> bool {
    let Some(cutoff) = cutoff else {
        return true;
    };
    let Some(raw) = r.published_date.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
        return true;
    };
    // 兼容 2026-08-03 / 2026-08-03T12:00:00Z 等
    let day = raw.get(0..10).unwrap_or(raw);
    if day.len() == 10 && day.chars().nth(4) == Some('-') {
        return day >= cutoff;
    }
    true
}

fn raw_hit_from_result(r: TavilyResult) -> RawHit {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let event_time = r
        .published_date
        .filter(|s| !s.is_empty())
        .unwrap_or(today);
    let summary = if r.content.chars().count() > 180 {
        format!("{}…", r.content.chars().take(180).collect::<String>())
    } else {
        r.content.clone()
    };
    let credibility = guess_credibility(&r.url);
    let title = if r.title.is_empty() {
        r.url.clone()
    } else {
        r.title
    };
    RawHit {
        source_id: "tavily".into(),
        title,
        summary,
        url: r.url,
        event_time,
        body: r.content,
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
    let name = if !req.partner.name.is_empty() {
        req.partner.name.clone()
    } else {
        req.subject_terms
            .first()
            .cloned()
            .unwrap_or_else(|| "合作机构".into())
    };
    format!("\"{name}\" {RISK_TAIL}")
}

fn partner_search_term(p: &Partner) -> String {
    let alias = p.alias.trim();
    // 有简称时优先用简称，便于一条 query 塞进更多机构；匹配阶段仍用完整词库
    if !alias.is_empty() && alias.chars().count() <= 16 {
        alias.to_string()
    } else {
        p.name.trim().to_string()
    }
}

fn build_multi_partner_query(partners: &[&Partner]) -> String {
    let names: Vec<String> = partners
        .iter()
        .map(|p| format!("\"{}\"", partner_search_term(p)))
        .filter(|s| s.len() > 2)
        .collect();
    if names.is_empty() {
        return format!("合作机构 {RISK_TAIL}");
    }
    format!("({}) {}", names.join(" OR "), RISK_TAIL)
}

/// 将名单打包进最多 `max_queries` 条检索式；装不下的部分靠游标下轮覆盖。
fn pack_partner_chunks<'a>(
    partners: &'a [Partner],
    start: usize,
    max_queries: usize,
    name_budget: usize,
) -> (Vec<Vec<&'a Partner>>, usize) {
    if partners.is_empty() || max_queries == 0 {
        return (Vec::new(), 0);
    }
    let n = partners.len();
    let start = start % n;
    let mut chunks: Vec<Vec<&Partner>> = Vec::new();
    let mut covered = 0usize;
    let mut qi = 0usize;

    while covered < n && qi < max_queries {
        let mut chunk: Vec<&Partner> = Vec::new();
        let mut used = 0usize;
        while covered < n {
            let p = &partners[(start + covered) % n];
            let term = partner_search_term(p);
            if term.is_empty() {
                covered += 1;
                continue;
            }
            // `"名" OR ` 开销约 +6
            let cost = term.chars().count() + 6;
            if !chunk.is_empty() && used + cost > name_budget {
                break;
            }
            if chunk.is_empty() && cost > name_budget {
                // 单名超长也硬塞一条，避免卡死
                chunk.push(p);
                covered += 1;
                break;
            }
            chunk.push(p);
            used += cost;
            covered += 1;
        }
        if chunk.is_empty() {
            break;
        }
        chunks.push(chunk);
        qi += 1;
    }
    (chunks, covered)
}

pub fn test_connection(cfg: &TavilyConfig) -> TavilyTestResult {
    if !cfg.ready() {
        return TavilyTestResult {
            ok: false,
            message: "请先启用并填写 Tavily API Key".into(),
            result_count: 0,
        };
    }
    match search_raw(
        &cfg.api_key,
        "Tavily API connectivity test",
        &SearchOpts::basic(1),
    ) {
        Ok(v) => {
            let n = v
                .get("results")
                .and_then(|r| r.as_array())
                .map(|a| a.len() as u32)
                .unwrap_or(0);
            TavilyTestResult {
                ok: true,
                message: format!("Tavily 连通正常，返回 {n} 条结果"),
                result_count: n,
            }
        }
        Err(e) => TavilyTestResult {
            ok: false,
            message: e,
            result_count: 0,
        },
    }
}

/// 单机构检索适配器（问鉴控/单查可用；跑批走 [`search_partners_bundled`]）。
#[allow(dead_code)]
pub struct TavilyAdapter {
    pub api_key: String,
    pub max_results: u32,
}

#[allow(dead_code)]
impl SourceAdapter for TavilyAdapter {
    fn source_id(&self) -> &'static str {
        "tavily"
    }

    fn search(&self, req: &SearchRequest) -> Result<Vec<RawHit>, SourceError> {
        if self.api_key.trim().is_empty() {
            return Err(SourceError::Blocked("Tavily API Key 未配置".into()));
        }
        let query = build_partner_query(req);
        search_hits_with_opts(
            &self.api_key,
            &query,
            &SearchWindow::LastDay.search_opts(self.max_results.min(5)),
        )
        .map_err(|e| SourceError::Blocked(e))
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

/// 跑批专用：把多家机构名打进最多 [`MAX_CALLS_PER_OPERATION`] 条 query 一起搜，
/// 再按词库把结果分配回各机构。名单过长装不下时游标轮转，下轮继续。
pub fn search_partners_bundled(
    conn: &Connection,
    api_key: &str,
    partners: &[Partner],
    max_results: u32,
    persist_cursor: bool,
    window: SearchWindow,
) -> Result<BundledSearchOutcome, String> {
    let mut out = BundledSearchOutcome::default();
    if partners.is_empty() {
        return Ok(out);
    }
    let n = partners.len();
    let cursor = db::get_setting(conn, KEY_BATCH_CURSOR)?
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0)
        % n;

    let (chunks, covered) = pack_partner_chunks(
        partners,
        cursor,
        MAX_CALLS_PER_OPERATION as usize,
        QUERY_NAME_BUDGET,
    );
    out.partners_covered = covered as u32;

    for chunk in &chunks {
        let query = build_multi_partner_query(chunk);
        out.queries.push(query.clone());
        out.calls += 1;
        let opts = window.search_opts(max_results.min(5));
        match search_hits_with_opts(api_key, &query, &opts) {
            Ok(hits) => {
                for hit in hits {
                    let text = format!("{} {} {}", hit.title, hit.summary, hit.body);
                    for p in chunk {
                        let matched = p.lexicon.iter().any(|t| {
                            let t = t.trim();
                            !t.is_empty() && text.contains(t)
                        }) || (!p.name.trim().is_empty() && text.contains(p.name.trim()))
                            || (!p.alias.trim().is_empty() && text.contains(p.alias.trim()));
                        if matched {
                            out.hits_by_partner
                                .entry(p.id.clone())
                                .or_default()
                                .push(hit.clone());
                        }
                    }
                }
            }
            Err(e) => {
                let names: Vec<&str> = chunk.iter().map(|p| p.name.as_str()).collect();
                out.errors
                    .push(format!("tavily @ [{}]: {e}", names.join("、")));
            }
        }
    }

    if persist_cursor {
        let next = (cursor + covered) % n;
        db::set_setting(conn, KEY_BATCH_CURSOR, &next.to_string())?;
    }
    Ok(out)
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

    #[test]
    fn last_day_uses_rolling_day_not_calendar_cutoff() {
        let opts = SearchWindow::LastDay.search_opts(5);
        assert_eq!(opts.time_range, Some("day"));
        assert!(opts.start_date.is_none());
        assert!(opts.drop_before_date.is_none());
        let week = SearchWindow::LastWeek.search_opts(5);
        assert_eq!(week.time_range, Some("week"));
        assert!(week.drop_before_date.is_none());
    }

    #[test]
    fn drops_stale_published_dates_for_today_filter() {
        let old = TavilyResult {
            title: "a".into(),
            url: "u".into(),
            content: "c".into(),
            published_date: Some("2020-01-01".into()),
        };
        let today = TavilyResult {
            title: "b".into(),
            url: "u".into(),
            content: "c".into(),
            published_date: Some("2026-08-03T08:00:00Z".into()),
        };
        let unknown = TavilyResult {
            title: "c".into(),
            url: "u".into(),
            content: "c".into(),
            published_date: None,
        };
        assert!(!keep_by_published_date(&old, Some("2026-08-03")));
        assert!(keep_by_published_date(&today, Some("2026-08-03")));
        assert!(keep_by_published_date(&unknown, Some("2026-08-03")));
    }

    #[test]
    fn packs_many_partners_into_at_most_two_queries() {
        let partners: Vec<Partner> = (0..12)
            .map(|i| demo_partner(&format!("p{i}"), &format!("测试机构{i:02}有限公司"), &format!("测企{i:02}")))
            .collect();
        let (chunks, covered) =
            pack_partner_chunks(&partners, 0, MAX_CALLS_PER_OPERATION as usize, QUERY_NAME_BUDGET);
        assert!(chunks.len() <= MAX_CALLS_PER_OPERATION as usize);
        assert!(covered >= 2);
        for chunk in &chunks {
            let q = build_multi_partner_query(chunk);
            assert!(q.contains("OR") || chunk.len() == 1);
            assert!(q.contains("处罚"));
        }
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
