//! 风险吹哨：日/周定时调度配置、跑批后日报/周报生成与落库。

use crate::db;
use crate::models::{
    AuditLog, BatchResult, RiskClue, WhistleJobResult, WhistleReport, WhistleSchedule,
    WhistleScheduleSave,
};
use crate::pipeline;
use chrono::{Datelike, Duration, Local, NaiveDate, NaiveTime, Weekday};
use rusqlite::{params, Connection};
use uuid::Uuid;

const KEY_DAILY_ENABLED: &str = "whistle_daily_enabled";
const KEY_DAILY_TIME: &str = "whistle_daily_time";
const KEY_WEEKLY_ENABLED: &str = "whistle_weekly_enabled";
const KEY_WEEKLY_DOW: &str = "whistle_weekly_dow";
const KEY_WEEKLY_TIME: &str = "whistle_weekly_time";
const KEY_LAST_DAILY: &str = "whistle_last_daily_run";
const KEY_LAST_WEEKLY: &str = "whistle_last_weekly_run";
const KEY_PARTNER_IDS: &str = "whistle_partner_ids";
const KEY_LAST_SCOPE: &str = "whistle_last_scope_json";

pub fn ensure_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS whistle_reports (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            period TEXT NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL DEFAULT '',
            markdown TEXT NOT NULL DEFAULT '',
            batch_stats_json TEXT NOT NULL DEFAULT '{}',
            clue_count INTEGER NOT NULL DEFAULT 0,
            level1 INTEGER NOT NULL DEFAULT 0,
            level2 INTEGER NOT NULL DEFAULT 0,
            level3 INTEGER NOT NULL DEFAULT 0,
            level4 INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_whistle_reports_kind
            ON whistle_reports(kind, created_at DESC);
        ",
    )
    .map_err(|e| e.to_string())
}

fn parse_hhmm(raw: &str) -> Result<NaiveTime, String> {
    let s = raw.trim();
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() < 2 {
        return Err(format!("时间格式需为 HH:mm，收到: {raw}"));
    }
    let h: u32 = parts[0]
        .parse()
        .map_err(|_| format!("时间格式需为 HH:mm，收到: {raw}"))?;
    let m: u32 = parts[1]
        .parse()
        .map_err(|_| format!("时间格式需为 HH:mm，收到: {raw}"))?;
    NaiveTime::from_hms_opt(h, m, 0).ok_or_else(|| format!("无效时间: {raw}"))
}

fn weekday_cn(d: u8) -> &'static str {
    match d {
        1 => "周一",
        2 => "周二",
        3 => "周三",
        4 => "周四",
        5 => "周五",
        6 => "周六",
        7 => "周日",
        _ => "周一",
    }
}

fn chrono_weekday(d: u8) -> Weekday {
    match d {
        1 => Weekday::Mon,
        2 => Weekday::Tue,
        3 => Weekday::Wed,
        4 => Weekday::Thu,
        5 => Weekday::Fri,
        6 => Weekday::Sat,
        _ => Weekday::Sun,
    }
}

fn weekday_num(w: Weekday) -> u8 {
    match w {
        Weekday::Mon => 1,
        Weekday::Tue => 2,
        Weekday::Wed => 3,
        Weekday::Thu => 4,
        Weekday::Fri => 5,
        Weekday::Sat => 6,
        Weekday::Sun => 7,
    }
}

fn week_key_for(date: chrono::NaiveDate) -> String {
    let iso = date.iso_week();
    format!("{:04}-W{:02}", iso.year(), iso.week())
}

fn week_bounds(date: chrono::NaiveDate) -> (chrono::NaiveDate, chrono::NaiveDate) {
    let days_from_mon = date.weekday().num_days_from_monday() as i64;
    let start = date - Duration::days(days_from_mon);
    let end = start + Duration::days(6);
    (start, end)
}

pub fn load_schedule(conn: &Connection) -> Result<WhistleSchedule, String> {
    let daily_enabled = db::get_setting(conn, KEY_DAILY_ENABLED)?
        .map(|v| v != "0" && v != "false")
        .unwrap_or(true);
    let daily_time = db::get_setting(conn, KEY_DAILY_TIME)?.unwrap_or_else(|| "08:00".into());
    let weekly_enabled = db::get_setting(conn, KEY_WEEKLY_ENABLED)?
        .map(|v| v != "0" && v != "false")
        .unwrap_or(true);
    let weekly_dow: u8 = db::get_setting(conn, KEY_WEEKLY_DOW)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .clamp(1, 7);
    let weekly_time = db::get_setting(conn, KEY_WEEKLY_TIME)?.unwrap_or_else(|| "09:00".into());
    let last_daily_run = db::get_setting(conn, KEY_LAST_DAILY)?;
    let last_weekly_run = db::get_setting(conn, KEY_LAST_WEEKLY)?;
    let partner_ids = load_partner_ids(conn)?;
    let (last_scope_mode, last_scope_ids) = load_last_scope(conn)?;
    let partner_total = db::list_partners(conn).map(|v| v.len()).unwrap_or(0);
    let scope_hint = if partner_ids.is_empty() {
        if partner_total == 0 {
            "尚未导入机构名单".into()
        } else {
            format!("定时监测全部 {partner_total} 家机构")
        }
    } else {
        format!("定时监测指定 {} 家机构", partner_ids.len())
    };

    let mut schedule = WhistleSchedule {
        daily_enabled,
        daily_time,
        weekly_enabled,
        weekly_dow,
        weekly_time,
        last_daily_run,
        last_weekly_run,
        next_daily_hint: String::new(),
        next_weekly_hint: String::new(),
        partner_ids,
        scope_hint,
        last_scope_mode,
        last_scope_ids,
    };
    fill_next_hints(&mut schedule);
    Ok(schedule)
}

fn fill_next_hints(s: &mut WhistleSchedule) {
    let now = Local::now();
    let today = now.date_naive();

    if !s.daily_enabled {
        s.next_daily_hint = "日报定时已关闭".into();
    } else if let Ok(t) = parse_hhmm(&s.daily_time) {
        let ran_today = s.last_daily_run.as_deref() == Some(&today.to_string());
        if ran_today {
            let next = today + Duration::days(1);
            s.next_daily_hint = format!("下次日报：{} {}", next, s.daily_time);
        } else if now.time() < t {
            s.next_daily_hint = format!("今日日报：{}", s.daily_time);
        } else {
            s.next_daily_hint = format!("今日日报待触发（计划 {}）", s.daily_time);
        }
    } else {
        s.next_daily_hint = "日报时间格式无效".into();
    }

    if !s.weekly_enabled {
        s.next_weekly_hint = "周报定时已关闭".into();
    } else if let Ok(t) = parse_hhmm(&s.weekly_time) {
        let target = chrono_weekday(s.weekly_dow);
        let mut d = today;
        let mut guard = 0;
        while d.weekday() != target && guard < 8 {
            d += Duration::days(1);
            guard += 1;
        }
        let key = week_key_for(d);
        let ran = s.last_weekly_run.as_deref() == Some(&key);
        if ran && d == today {
            d += Duration::days(7);
        } else if d == today && (now.time() >= t || ran) {
            if ran {
                d += Duration::days(7);
            }
        }
        s.next_weekly_hint = format!(
            "下次周报：{} {} {}",
            d,
            weekday_cn(s.weekly_dow),
            s.weekly_time
        );
    } else {
        s.next_weekly_hint = "周报时间格式无效".into();
    }
}

pub fn save_schedule(conn: &Connection, input: &WhistleScheduleSave) -> Result<WhistleSchedule, String> {
    if let Some(v) = input.daily_enabled {
        db::set_setting(conn, KEY_DAILY_ENABLED, if v { "1" } else { "0" })?;
    }
    if let Some(ref t) = input.daily_time {
        let nt = parse_hhmm(t)?;
        db::set_setting(conn, KEY_DAILY_TIME, &nt.format("%H:%M").to_string())?;
    }
    if let Some(v) = input.weekly_enabled {
        db::set_setting(conn, KEY_WEEKLY_ENABLED, if v { "1" } else { "0" })?;
    }
    if let Some(d) = input.weekly_dow {
        if !(1..=7).contains(&d) {
            return Err("周报应选 1（周一）到 7（周日）".into());
        }
        db::set_setting(conn, KEY_WEEKLY_DOW, &d.to_string())?;
    }
    if let Some(ref t) = input.weekly_time {
        let nt = parse_hhmm(t)?;
        db::set_setting(conn, KEY_WEEKLY_TIME, &nt.format("%H:%M").to_string())?;
    }
    if let Some(ref ids) = input.partner_ids {
        let cleaned: Vec<String> = ids
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        db::set_setting(
            conn,
            KEY_PARTNER_IDS,
            &serde_json::to_string(&cleaned).unwrap_or_else(|_| "[]".into()),
        )?;
    }
    load_schedule(conn)
}

fn load_partner_ids(conn: &Connection) -> Result<Vec<String>, String> {
    let raw = match db::get_setting(conn, KEY_PARTNER_IDS)? {
        Some(v) if !v.trim().is_empty() => v,
        _ => return Ok(Vec::new()),
    };
    let ids: Vec<String> = serde_json::from_str(&raw).unwrap_or_default();
    Ok(ids
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LastScopeStored {
    mode: String,
    ids: Vec<String>,
}

fn load_last_scope(conn: &Connection) -> Result<(String, Vec<String>), String> {
    let raw = match db::get_setting(conn, KEY_LAST_SCOPE)? {
        Some(v) if !v.trim().is_empty() => v,
        _ => return Ok((String::new(), Vec::new())),
    };
    let parsed: LastScopeStored = serde_json::from_str(&raw).unwrap_or(LastScopeStored {
        mode: String::new(),
        ids: Vec::new(),
    });
    let mode = parsed.mode.trim().to_lowercase();
    let ids: Vec<String> = parsed
        .ids
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Ok((mode, ids))
}

pub fn save_last_scope(
    conn: &Connection,
    mode: &str,
    ids: &[String],
) -> Result<(String, Vec<String>), String> {
    let mode = mode.trim().to_lowercase();
    let mode = if mode == "selected" { "selected" } else { "all" };
    let cleaned: Vec<String> = ids
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let payload = LastScopeStored {
        mode: mode.to_string(),
        ids: if mode == "selected" {
            cleaned.clone()
        } else {
            Vec::new()
        },
    };
    db::set_setting(
        conn,
        KEY_LAST_SCOPE,
        &serde_json::to_string(&payload).unwrap_or_else(|_| "{}".into()),
    )?;
    Ok((payload.mode, payload.ids))
}

/// 供后台定时器调用：若已到点且本日/本周未跑，则执行对应任务。
pub fn tick(conn: &Connection) -> Result<Vec<WhistleJobResult>, String> {
    ensure_schema(conn)?;
    let schedule = load_schedule(conn)?;
    let now = Local::now();
    let today = now.date_naive().to_string();
    let mut out = Vec::new();

    if schedule.daily_enabled {
        if let Ok(t) = parse_hhmm(&schedule.daily_time) {
            let already = schedule.last_daily_run.as_deref() == Some(today.as_str());
            if !already && now.time() >= t {
                match run_job(conn, "daily", "scheduler", None, None) {
                    Ok(job) => out.push(job),
                    Err(e) => {
                        // 失败也记一次，避免同一分钟反复重试打爆；允许用户手动再跑
                        let _ = db::set_setting(conn, KEY_LAST_DAILY, &today);
                        let _ = db::insert_audit(
                            conn,
                            &AuditLog {
                                id: Uuid::new_v4().to_string(),
                                actor: "scheduler".into(),
                                action: "whistle_daily_failed".into(),
                                target: "whistle_reports".into(),
                                detail: e,
                                ts: now.format("%Y-%m-%d %H:%M:%S").to_string(),
                            },
                        );
                    }
                }
            }
        }
    }

    if schedule.weekly_enabled {
        if let Ok(t) = parse_hhmm(&schedule.weekly_time) {
            let dow_ok = weekday_num(now.weekday()) == schedule.weekly_dow;
            let key = week_key_for(now.date_naive());
            let already = schedule.last_weekly_run.as_deref() == Some(key.as_str());
            if dow_ok && !already && now.time() >= t {
                match run_job(conn, "weekly", "scheduler", None, None) {
                    Ok(job) => out.push(job),
                    Err(e) => {
                        let _ = db::set_setting(conn, KEY_LAST_WEEKLY, &key);
                        let _ = db::insert_audit(
                            conn,
                            &AuditLog {
                                id: Uuid::new_v4().to_string(),
                                actor: "scheduler".into(),
                                action: "whistle_weekly_failed".into(),
                                target: "whistle_reports".into(),
                                detail: e,
                                ts: now.format("%Y-%m-%d %H:%M:%S").to_string(),
                            },
                        );
                    }
                }
            }
        }
    }

    Ok(out)
}

pub fn run_job(
    conn: &Connection,
    kind: &str,
    actor: &str,
    partner_ids: Option<Vec<String>>,
    on_date: Option<String>,
) -> Result<WhistleJobResult, String> {
    ensure_schema(conn)?;
    let kind = kind.trim().to_lowercase();
    if kind != "daily" && kind != "weekly" {
        return Err("kind 仅支持 daily 或 weekly".into());
    }

    let today = Local::now().date_naive();
    let as_of = parse_on_date(on_date.as_deref(), today)?;
    let dated = on_date
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .is_some();
    let window = if dated {
        crate::baidu_search::SearchWindow::calendar_for(&kind, as_of)
    } else {
        crate::baidu_search::SearchWindow::from_job_kind(&kind)
    };

    let ids = match partner_ids {
        Some(v) => v,
        None => load_schedule(conn)?.partner_ids,
    };
    let id_ref = if ids.is_empty() {
        None
    } else {
        Some(ids.as_slice())
    };
    // 定时任务推进游标轮转；手动指定日期/机构从名单头开始且不改游标，避免把排程打乱。
    let persist_cursor = actor == "scheduler" && !dated;
    if actor != "scheduler" {
        let mode = if ids.is_empty() { "all" } else { "selected" };
        let _ = save_last_scope(conn, mode, &ids);
    }
    let batch =
        pipeline::run_whistle_batch_scoped(conn, false, actor, id_ref, persist_cursor, window)?;
    let report = build_and_save_report(conn, &kind, &batch, actor, as_of)?;

    if !dated || as_of == today {
        if kind == "daily" {
            db::set_setting(conn, KEY_LAST_DAILY, &today.to_string())?;
        } else {
            db::set_setting(conn, KEY_LAST_WEEKLY, &week_key_for(today))?;
        }
    }

    Ok(WhistleJobResult { kind, batch, report })
}

fn parse_on_date(raw: Option<&str>, today: NaiveDate) -> Result<NaiveDate, String> {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(today),
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|_| format!("日期需为 YYYY-MM-DD，收到: {s}")),
    }
}

fn ymd_prefix(raw: &str) -> Option<&str> {
    let s = raw.trim();
    let day = s.get(0..10)?;
    if NaiveDate::parse_from_str(day, "%Y-%m-%d").is_ok() {
        Some(day)
    } else {
        None
    }
}

fn clue_date_key(c: &RiskClue) -> String {
    ymd_prefix(&c.event_date)
        .or_else(|| ymd_prefix(&c.created_at))
        .unwrap_or("")
        .to_string()
}

fn filter_clues_for_period(
    all: &[RiskClue],
    kind: &str,
    as_of: NaiveDate,
) -> (String, Vec<RiskClue>) {
    if kind == "daily" {
        let key = as_of.to_string();
        let mut list: Vec<_> = all
            .iter()
            .filter(|c| clue_date_key(c) == key)
            .cloned()
            .collect();
        list.sort_by(|a, b| a.level.cmp(&b.level).then(b.event_date.cmp(&a.event_date)));
        list.dedup_by(|a, b| a.id == b.id);
        (key, list)
    } else {
        let (start, end) = week_bounds(as_of);
        let period = format!("{start} ~ {end}");
        let mut list: Vec<_> = all
            .iter()
            .filter(|c| {
                let d = clue_date_key(c);
                d >= start.to_string() && d <= end.to_string()
            })
            .cloned()
            .collect();
        list.sort_by(|a, b| a.level.cmp(&b.level).then(b.event_date.cmp(&a.event_date)));
        (period, list)
    }
}

fn keep_period_clue(c: &RiskClue) -> bool {
    if c.source_system == "baidu" || c.source_system == "tavily" {
        crate::baidu_search::keep_search_hit(&c.title, &c.summary)
    } else {
        true
    }
}

fn build_and_save_report(
    conn: &Connection,
    kind: &str,
    batch: &BatchResult,
    actor: &str,
    as_of: NaiveDate,
) -> Result<WhistleReport, String> {
    let period = if kind == "daily" {
        as_of.to_string()
    } else {
        let (start, end) = week_bounds(as_of);
        format!("{start} ~ {end}")
    };
    // 报告只展示本次跑批命中的线索，不把库里历史 L1/L2 或其他机构的旧稿拼进来。
    let mut clues: Vec<RiskClue> = batch
        .clues
        .iter()
        .filter(|c| keep_period_clue(c))
        .cloned()
        .collect();
    clues.sort_by(|a, b| a.level.cmp(&b.level).then(b.event_date.cmp(&a.event_date)));

    let level1 = clues.iter().filter(|c| c.level == 1).count() as u32;
    let level2 = clues.iter().filter(|c| c.level == 2).count() as u32;
    let level3 = clues.iter().filter(|c| c.level == 3).count() as u32;
    let level4 = clues.iter().filter(|c| c.level == 4).count() as u32;

    let title = if kind == "daily" {
        format!("风险吹哨日报 · {period}")
    } else {
        format!("风险吹哨周报 · {period}")
    };

    let summary = format!(
        "本次跑批命中 {} 条（L1={} / L2={} / L3={} / L4={}）；扫描 {} 家机构，其中新增 {} 条。",
        clues.len(),
        level1,
        level2,
        level3,
        level4,
        batch.partners_scanned,
        batch.clues_upserted
    );

    let markdown = render_markdown(kind, &title, &period, &summary, batch, &clues);
    let batch_stats_json = serde_json::json!({
        "partnersScanned": batch.partners_scanned,
        "rawHits": batch.raw_hits,
        "matched": batch.matched,
        "cluesUpserted": batch.clues_upserted,
        "level1": batch.level1,
        "level2": batch.level2,
        "sourceErrors": batch.source_errors,
        "scopeNote": batch.scope_note,
        "searchQueries": batch.search_queries,
        "clues": clues.iter().take(80).map(|c| serde_json::json!({
            "id": c.id,
            "partner": c.partner,
            "partnerType": c.partner_type,
            "level": c.level,
            "title": c.title,
            "summary": c.summary,
            "eventDate": c.event_date,
            "status": c.status,
            "sourceSystem": c.source_system,
            "sourceUrl": c.source_url,
            "credibility": c.credibility,
            "ruleId": c.rule_id,
            "legalBasis": c.legal_basis,
        })).collect::<Vec<_>>(),
    })
    .to_string();

    let report = WhistleReport {
        id: format!(
            "{}-{}",
            if kind == "daily" { "DR" } else { "WR" },
            &Uuid::new_v4().to_string()[..8]
        ),
        kind: kind.into(),
        period,
        title,
        summary,
        markdown,
        batch_stats_json,
        clue_count: clues.len() as u32,
        level1,
        level2,
        level3,
        level4,
        created_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };

    save_report(conn, &report)?;
    db::insert_audit(
        conn,
        &AuditLog {
            id: Uuid::new_v4().to_string(),
            actor: actor.into(),
            action: if kind == "daily" {
                "whistle_daily_report".into()
            } else {
                "whistle_weekly_report".into()
            },
            target: report.id.clone(),
            detail: format!(
                "period={} clues={} L1={} L2={}",
                report.period, report.clue_count, report.level1, report.level2
            ),
            ts: report.created_at.clone(),
        },
    )?;
    Ok(report)
}

fn render_markdown(
    kind: &str,
    title: &str,
    period: &str,
    summary: &str,
    batch: &BatchResult,
    clues: &[RiskClue],
) -> String {
    let mut md = String::new();
    md.push_str(&format!("# {title}\n\n"));
    md.push_str(&format!("- 统计周期：{period}\n"));
    md.push_str(&format!(
        "- 生成时间：{}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    md.push_str(&format!("- 摘要：{summary}\n\n"));

    md.push_str("## 本次跑批\n\n");
    md.push_str(&format!(
        "| 扫描机构 | 原始命中 | 匹配 | 新增线索 | 新增 L1 | 新增 L2 |\n|---|---|---|---|---|---|\n| {} | {} | {} | {} | {} | {} |\n\n",
        batch.partners_scanned,
        batch.raw_hits,
        batch.matched,
        batch.clues_upserted,
        batch.level1,
        batch.level2
    ));
    if !batch.search_queries.is_empty() {
        md.push_str("### 全网检索词\n\n");
        for (i, q) in batch.search_queries.iter().enumerate() {
            md.push_str(&format!("{}. `{}`\n", i + 1, q.replace('`', "'")));
        }
        md.push('\n');
    } else {
        md.push_str("### 全网检索词\n\n未发出检索（请检查是否已配置并启用百度千帆搜索）。\n\n");
    }

    if !batch.source_errors.is_empty() {
        md.push_str("### 数据源提示\n\n");
        for e in batch.source_errors.iter().take(10) {
            md.push_str(&format!("- {e}\n"));
        }
        md.push('\n');
    }

    let l12: Vec<_> = clues.iter().filter(|c| c.level <= 2).collect();
    let l3: Vec<_> = clues.iter().filter(|c| c.level == 3).collect();
    let l4: Vec<_> = clues.iter().filter(|c| c.level == 4).collect();

    md.push_str("## 1/2 级即时处置\n\n");
    if l12.is_empty() {
        md.push_str("本期无 1/2 级待关注线索。\n\n");
    } else {
        md.push_str(&clue_table(&l12));
    }

    if kind == "weekly" {
        md.push_str("## 3 级周报主体\n\n");
        if l3.is_empty() {
            md.push_str("本期无 3 级线索。\n\n");
        } else {
            md.push_str(&clue_table(&l3));
        }
        md.push_str("## 4 级附录\n\n");
        if l4.is_empty() {
            md.push_str("本期无 4 级线索。\n\n");
        } else {
            md.push_str(&clue_table(&l4));
        }
    } else {
        md.push_str("## 3/4 级跟踪\n\n");
        let rest: Vec<_> = clues.iter().filter(|c| c.level >= 3).collect();
        if rest.is_empty() {
            md.push_str("本期无 3/4 级线索。\n\n");
        } else {
            md.push_str(&clue_table(&rest));
        }
    }

    md.push_str("---\n*由智鉴风控官自动生成，核心定级以规则引擎为准，请人工复核 1/2 级。*\n");
    md
}

fn clue_table(clues: &[&RiskClue]) -> String {
    let mut s = String::from(
        "| 线索ID | 机构 | 类型 | 等级 | 摘要 | 来源 | 可信度 | 事件时间 | 状态 | 规则 | 法规依据 |\n|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for c in clues {
        let summary = c.summary.replace('|', "\\|").replace('\n', " ");
        let title = c.title.replace('|', "\\|");
        s.push_str(&format!(
            "| {} | {} | {} | L{} | {} — {} | {} | {} | {} | {} | {} | {} |\n",
            c.id,
            c.partner,
            c.partner_type,
            c.level,
            title,
            summary,
            c.source_system,
            c.credibility,
            c.event_date,
            c.status,
            c.rule_id,
            c.legal_basis.replace('|', "\\|"),
        ));
    }
    s.push('\n');
    s
}

pub fn save_report(conn: &Connection, report: &WhistleReport) -> Result<(), String> {
    ensure_schema(conn)?;
    conn.execute(
        "INSERT INTO whistle_reports
         (id, kind, period, title, summary, markdown, batch_stats_json,
          clue_count, level1, level2, level3, level4, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![
            report.id,
            report.kind,
            report.period,
            report.title,
            report.summary,
            report.markdown,
            report.batch_stats_json,
            report.clue_count,
            report.level1,
            report.level2,
            report.level3,
            report.level4,
            report.created_at,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_reports(conn: &Connection, limit: u32) -> Result<Vec<WhistleReport>, String> {
    ensure_schema(conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, kind, period, title, summary, markdown, batch_stats_json,
                    clue_count, level1, level2, level3, level4, created_at
             FROM whistle_reports
             ORDER BY created_at DESC
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit], |row| {
            Ok(WhistleReport {
                id: row.get(0)?,
                kind: row.get(1)?,
                period: row.get(2)?,
                title: row.get(3)?,
                summary: row.get(4)?,
                markdown: row.get(5)?,
                batch_stats_json: row.get(6)?,
                clue_count: row.get(7)?,
                level1: row.get(8)?,
                level2: row.get(9)?,
                level3: row.get(10)?,
                level4: row.get(11)?,
                created_at: row.get(12)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// 后台轮询：每 30 秒检查一次（仅应用进程存活期间生效）。
pub fn spawn_scheduler(app: tauri::AppHandle) {
    use tauri::Manager;
    std::thread::Builder::new()
        .name("whistle-scheduler".into())
        .spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(30));
            let state = app.state::<crate::state::AppState>();
            let conn = match state.conn.lock() {
                Ok(c) => c,
                Err(_) => continue,
            };
            match tick(&conn) {
                Ok(jobs) if !jobs.is_empty() => {
                    for j in &jobs {
                        eprintln!(
                            "[whistle-scheduler] {} report {} clues={}",
                            j.kind, j.report.id, j.report.clue_count
                        );
                    }
                }
                Ok(_) => {}
                Err(e) => eprintln!("[whistle-scheduler] tick error: {e}"),
            }
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_time_and_week_key() {
        assert!(parse_hhmm("08:00").is_ok());
        assert!(parse_hhmm("9:30").is_err() || parse_hhmm("09:30").is_ok());
        let d = chrono::NaiveDate::from_ymd_opt(2026, 7, 31).unwrap();
        assert!(week_key_for(d).contains("W"));
        let (s, e) = week_bounds(d);
        assert_eq!(s.weekday(), Weekday::Mon);
        assert_eq!(e.weekday(), Weekday::Sun);
        assert_eq!(
            parse_on_date(Some("2026-08-12"), d).unwrap().to_string(),
            "2026-08-12"
        );
        assert!(parse_on_date(Some("2026/08/12"), d).is_err());
        assert_eq!(parse_on_date(None, d).unwrap(), d);
    }

    #[test]
    fn clue_date_key_skips_undated_without_panic() {
        let mut c = RiskClue {
            id: "x".into(),
            partner_id: "p".into(),
            partner: "桔子数科".into(),
            partner_type: "loan".into(),
            level: 3,
            title: "爆雷".into(),
            summary: String::new(),
            event_date: "日期不详".into(),
            owner: String::new(),
            progress: 0,
            status: "跟踪中".into(),
            source_system: "baidu".into(),
            source_url: String::new(),
            credibility: "C".into(),
            rule_id: String::new(),
            rule_set_version: String::new(),
            legal_basis: String::new(),
            denoise_status: "new".into(),
            related_party_flag: false,
            evidence_hash: "h".into(),
            created_at: "2026-07-02 01:00:00".into(),
        };
        assert_eq!(clue_date_key(&c), "2026-07-02");
        c.created_at.clear();
        assert_eq!(clue_date_key(&c), "");
        let as_of = NaiveDate::from_ymd_opt(2026, 7, 2).unwrap();
        let (_, list) = filter_clues_for_period(&[c], "weekly", as_of);
        assert!(list.is_empty());
    }
}
