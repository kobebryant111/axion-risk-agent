//! 经营守护：财报指标计算、四维评级、融担专项、同业/时序对比与季报落库。

use crate::db;
use crate::llm::{self, ChatMessage, LlmConfig};
use crate::models::{
    DimensionScore, FinanceAnalyzeRequest, FinanceBatchResult, FinanceCsvImportResult,
    FinanceMetrics, FinancePeerRow, FinanceReport, FinanceSeriesPoint, GuaranteeSpecial,
};
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn ensure_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS finance_reports (
            id TEXT PRIMARY KEY,
            partner_id TEXT NOT NULL DEFAULT '',
            partner_name TEXT NOT NULL,
            partner_type TEXT NOT NULL DEFAULT '',
            period TEXT NOT NULL,
            metrics_json TEXT NOT NULL,
            scores_json TEXT NOT NULL,
            overall_rating TEXT NOT NULL,
            concerns_json TEXT NOT NULL DEFAULT '[]',
            summary TEXT NOT NULL DEFAULT '',
            markdown TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            risk_score INTEGER NOT NULL DEFAULT 0,
            source_kind TEXT NOT NULL DEFAULT '',
            related_flags_json TEXT NOT NULL DEFAULT '[]'
        );
        CREATE INDEX IF NOT EXISTS idx_finance_partner ON finance_reports(partner_id);
        ",
    )
    .map_err(|e| e.to_string())?;
    let _ = conn.execute(
        "ALTER TABLE finance_reports ADD COLUMN risk_score INTEGER NOT NULL DEFAULT 0",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE finance_reports ADD COLUMN source_kind TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE finance_reports ADD COLUMN related_flags_json TEXT NOT NULL DEFAULT '[]'",
        [],
    );
    let _ = conn.execute(
        "DELETE FROM finance_reports WHERE source_kind = 'demo' OR partner_name LIKE '%演示样例%'",
        [],
    );
    Ok(())
}

pub fn demo_lexin_metrics() -> FinanceMetrics {
    FinanceMetrics {
        revenue: Some(131.52),
        revenue_yoy: Some(-7.4),
        net_profit: Some(16.77),
        net_profit_yoy: Some(52.4),
        net_margin: Some(12.75),
        gross_margin: Some(71.45),
        op_margin: Some(17.70),
        roa: Some(7.33),
        roe: Some(14.66),
        asset_liability_ratio: Some(48.40),
        current_ratio: Some(1.86),
        quick_ratio: Some(1.86),
        equity_ratio: Some(0.52),
        debt_ebitda: Some(2.01),
        asset_turnover: Some(0.58),
        loan_originated: Some(2050.0),
        loan_balance: Some(966.0),
        npl_90: None,
        ocfo: Some(36.0),
        fcf: Some(33.0),
        capex_ratio: Some(2.68),
        dividend_payout: Some(23.26),
        guarantee: None,
        currency_unit: "亿元人民币".into(),
        notes: "演示样例：乐信 2025 全年（规划文档标定数据，非生产唯一模板）".into(),
    }
}

pub fn analyze(req: &FinanceAnalyzeRequest) -> Result<FinanceReport, String> {
    let metrics = if req.use_demo.unwrap_or(false) {
        demo_lexin_metrics()
    } else {
        req.metrics.clone().ok_or("请提供财务指标，或勾选载入演示样例")?
    };

    let (scores, concerns) = score_metrics(&metrics, &req.partner_type);
    let overall = overall_rating(&scores, &concerns);
    let risk_score = quantitative_score(&scores, &concerns);
    let source_kind = req
        .source_kind
        .clone()
        .unwrap_or_else(|| {
            if req.use_demo.unwrap_or(false) {
                "demo".into()
            } else if req.document_text.as_ref().is_some_and(|s| !s.trim().is_empty()) {
                "text".into()
            } else {
                "metrics".into()
            }
        });
    let summary = build_summary(
        &req.partner_name,
        &req.period,
        &overall,
        risk_score,
        &concerns,
    );
    let markdown = render_markdown(
        &req.partner_name,
        &req.partner_type,
        &req.period,
        &metrics,
        &scores,
        &overall,
        risk_score,
        &concerns,
        &[],
        None,
        None,
        &summary,
        &source_kind,
    );

    Ok(FinanceReport {
        id: format!("FR-{}", &Uuid::new_v4().to_string()[..8]),
        partner_id: req.partner_id.clone().unwrap_or_default(),
        partner_name: req.partner_name.clone(),
        partner_type: req.partner_type.clone(),
        period: req.period.clone(),
        metrics,
        scores,
        overall_rating: overall,
        risk_score,
        concerns,
        related_flags: Vec::new(),
        summary,
        markdown,
        source_kind,
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    })
}

/// 关联方穿透 + 同业/时序对比章节，并重写 markdown。
pub fn enrich_report(conn: &Connection, report: &mut FinanceReport) -> Result<(), String> {
    report.related_flags = related_party_flags(conn, &report.partner_id, &report.partner_name)?;
    let peers = compare_peers(conn, &report.partner_type, &report.period, 12)?;
    let series = list_series(conn, &report.partner_id, &report.partner_name, 8)?;
    report.markdown = render_markdown(
        &report.partner_name,
        &report.partner_type,
        &report.period,
        &report.metrics,
        &report.scores,
        &report.overall_rating,
        report.risk_score,
        &report.concerns,
        &report.related_flags,
        Some(&peers),
        Some(&series),
        &report.summary,
        &report.source_kind,
    );
    Ok(())
}

fn quantitative_score(scores: &[DimensionScore], concerns: &[String]) -> i32 {
    let avg = if scores.is_empty() {
        70.0
    } else {
        scores.iter().map(|s| s.score as f64).sum::<f64>() / scores.len() as f64
    };
    let mut s = avg.round() as i32;
    let red = concerns.iter().any(|c| {
        c.contains("超红线") || c.contains("不合规") || c.contains("造假") || c.contains("致命")
    });
    if red {
        s = s.min(55);
    }
    s.clamp(0, 100)
}

fn related_party_flags(
    conn: &Connection,
    partner_id: &str,
    partner_name: &str,
) -> Result<Vec<String>, String> {
    let partners = db::list_partners(conn)?;
    let partner = partners.iter().find(|p| {
        (!partner_id.is_empty() && p.id == partner_id) || p.name == partner_name
    });
    let Some(p) = partner else {
        return Ok(vec![
            "未绑定机构名单：无法自动穿透股东/关联方，请先在机构名单维护关联方。".into(),
        ]);
    };

    let mut flags = Vec::new();
    if p.related_parties.is_empty() {
        flags.push("关联方名单为空：建议补充股东/关联企业以便穿透核查。".into());
    } else {
        flags.push(format!(
            "已登记关联方 {} 家：{}",
            p.related_parties.len(),
            p.related_parties.iter().take(6).cloned().collect::<Vec<_>>().join("、")
        ));
        let clues = db::list_clues(conn)?;
        for rel in &p.related_parties {
            let hits: Vec<_> = clues
                .iter()
                .filter(|c| {
                    c.level <= 2
                        && (c.partner.contains(rel.as_str())
                            || c.title.contains(rel.as_str())
                            || c.related_party_flag)
                })
                .take(3)
                .collect();
            if !hits.is_empty() {
                flags.push(format!(
                    "关联方「{rel}」存在 1/2 级风险线索 {} 条（示例：{}）",
                    hits.len(),
                    hits[0].title
                ));
            }
        }
        if flags.len() == 1 {
            flags.push("本地线索库暂未见关联方 1/2 级传导命中；建议结合企查查/天眼查补证股权与关联交易。".into());
        }
    }
    Ok(flags)
}

pub fn save_report(conn: &Connection, report: &FinanceReport) -> Result<(), String> {
    ensure_schema(conn)?;
    // 同一机构同一期间覆盖，保证历史库可按期间对齐做时序/同业
    if !report.partner_id.is_empty() {
        let _ = conn.execute(
            "DELETE FROM finance_reports WHERE partner_id = ?1 AND period = ?2",
            params![report.partner_id, report.period],
        );
    } else {
        let _ = conn.execute(
            "DELETE FROM finance_reports WHERE partner_id = '' AND partner_name = ?1 AND period = ?2",
            params![report.partner_name, report.period],
        );
    }
    conn.execute(
        "INSERT INTO finance_reports
         (id, partner_id, partner_name, partner_type, period, metrics_json, scores_json,
          overall_rating, concerns_json, summary, markdown, created_at,
          risk_score, source_kind, related_flags_json)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        params![
            report.id,
            report.partner_id,
            report.partner_name,
            report.partner_type,
            report.period,
            serde_json::to_string(&report.metrics).unwrap_or_else(|_| "{}".into()),
            serde_json::to_string(&report.scores).unwrap_or_else(|_| "[]".into()),
            report.overall_rating,
            serde_json::to_string(&report.concerns).unwrap_or_else(|_| "[]".into()),
            report.summary,
            report.markdown,
            report.created_at,
            report.risk_score,
            report.source_kind,
            serde_json::to_string(&report.related_flags).unwrap_or_else(|_| "[]".into()),
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn map_report_row(
    id: String,
    pid: String,
    name: String,
    ptype: String,
    period: String,
    mj: String,
    sj: String,
    rating: String,
    cj: String,
    summary: String,
    md: String,
    ts: String,
    risk_score: i32,
    source_kind: String,
    related_json: String,
) -> FinanceReport {
    FinanceReport {
        id,
        partner_id: pid,
        partner_name: name,
        partner_type: ptype,
        period,
        metrics: serde_json::from_str(&mj).unwrap_or_default(),
        scores: serde_json::from_str(&sj).unwrap_or_default(),
        overall_rating: rating,
        risk_score,
        concerns: serde_json::from_str(&cj).unwrap_or_default(),
        related_flags: serde_json::from_str(&related_json).unwrap_or_default(),
        summary,
        markdown: md,
        source_kind,
        created_at: ts,
    }
}

pub fn list_reports(conn: &Connection, limit: u32) -> Result<Vec<FinanceReport>, String> {
    ensure_schema(conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, partner_id, partner_name, partner_type, period, metrics_json, scores_json,
                    overall_rating, concerns_json, summary, markdown, created_at,
                    risk_score, source_kind, related_flags_json
             FROM finance_reports ORDER BY created_at DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit as i64], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, String>(8)?,
                r.get::<_, String>(9)?,
                r.get::<_, String>(10)?,
                r.get::<_, String>(11)?,
                r.get::<_, i32>(12).unwrap_or(0),
                r.get::<_, String>(13).unwrap_or_default(),
                r.get::<_, String>(14).unwrap_or_else(|_| "[]".into()),
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for row in rows {
        let (id, pid, name, ptype, period, mj, sj, rating, cj, summary, md, ts, rs, sk, rj) =
            row.map_err(|e| e.to_string())?;
        out.push(map_report_row(
            id, pid, name, ptype, period, mj, sj, rating, cj, summary, md, ts, rs, sk, rj,
        ));
    }
    Ok(out)
}

pub fn compare_peers(
    conn: &Connection,
    partner_type: &str,
    period: &str,
    limit: u32,
) -> Result<Vec<FinancePeerRow>, String> {
    ensure_schema(conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, partner_id, partner_name, partner_type, period, overall_rating, risk_score
             FROM finance_reports
             WHERE partner_type = ?1 AND period = ?2
             ORDER BY risk_score DESC, created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![partner_type, period], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, i32>(6).unwrap_or(0),
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut seen = std::collections::HashSet::new();
    let mut peers = Vec::new();
    for row in rows {
        let (id, pid, name, ptype, per, rating, score) = row.map_err(|e| e.to_string())?;
        let key = if pid.is_empty() {
            name.clone()
        } else {
            pid.clone()
        };
        if !seen.insert(key) {
            continue;
        }
        peers.push(FinancePeerRow {
            partner_id: pid,
            partner_name: name,
            partner_type: ptype,
            period: per,
            overall_rating: rating,
            risk_score: score,
            rank: 0,
            report_id: id,
        });
        if peers.len() as u32 >= limit {
            break;
        }
    }
    for (i, p) in peers.iter_mut().enumerate() {
        p.rank = (i + 1) as u32;
    }
    Ok(peers)
}

pub fn list_series(
    conn: &Connection,
    partner_id: &str,
    partner_name: &str,
    limit: u32,
) -> Result<Vec<FinanceSeriesPoint>, String> {
    ensure_schema(conn)?;
    let reports = list_reports(conn, 200)?;
    let mut series: Vec<_> = reports
        .into_iter()
        .filter(|r| {
            (!partner_id.is_empty() && r.partner_id == partner_id)
                || (!partner_name.is_empty() && r.partner_name == partner_name)
        })
        .map(|r| FinanceSeriesPoint {
            period: r.period,
            risk_score: r.risk_score,
            overall_rating: r.overall_rating,
            asset_liability_ratio: r.metrics.asset_liability_ratio,
            net_margin: r.metrics.net_margin,
            fcf: r.metrics.fcf,
            report_id: r.id,
            created_at: r.created_at,
        })
        .collect();
    // 期间近似升序（字符串）；同期间保留最新
    series.sort_by(|a, b| a.period.cmp(&b.period).then(a.created_at.cmp(&b.created_at)));
    let mut seen = std::collections::HashSet::new();
    let mut uniq = Vec::new();
    for p in series.into_iter().rev() {
        if seen.insert(p.period.clone()) {
            uniq.push(p);
        }
    }
    uniq.reverse();
    if uniq.len() > limit as usize {
        let start = uniq.len() - limit as usize;
        uniq = uniq[start..].to_vec();
    }
    Ok(uniq)
}

/// 季度批量：对名单内机构，复用其最近一期指标重评到目标期间；无历史则跳过（可选演示填乐信）。
pub fn run_quarter_batch(
    conn: &Connection,
    period: &str,
    fill_demo_if_empty: bool,
) -> Result<FinanceBatchResult, String> {
    ensure_schema(conn)?;
    let partners = db::list_partners(conn)?;
    let mut analyzed = 0u32;
    let mut skipped = 0u32;
    let mut failed = 0u32;
    let mut errors = Vec::new();
    let mut reports = Vec::new();

    for p in &partners {
        let hist = list_series(conn, &p.id, &p.name, 1)?;
        let metrics = if let Some(last) = hist.first() {
            // 取最近报告完整 metrics
            list_reports(conn, 80)?
                .into_iter()
                .find(|r| r.id == last.report_id)
                .map(|r| r.metrics)
        } else if fill_demo_if_empty && (p.name.contains("乐信") || partners.len() == 1) {
            Some(demo_lexin_metrics())
        } else {
            None
        };

        let Some(metrics) = metrics else {
            skipped += 1;
            continue;
        };

        let req = FinanceAnalyzeRequest {
            partner_id: Some(p.id.clone()),
            partner_name: p.name.clone(),
            partner_type: p.partner_type.clone(),
            period: period.to_string(),
            metrics: Some(metrics),
            use_demo: Some(false),
            document_text: None,
            source_kind: Some("batch".into()),
        };
        match analyze(&req) {
            Ok(mut report) => {
                if let Err(e) = enrich_report(conn, &mut report) {
                    errors.push(format!("{} enrich: {e}", p.name));
                }
                if let Err(e) = save_report(conn, &report) {
                    failed += 1;
                    errors.push(format!("{} save: {e}", p.name));
                } else {
                    analyzed += 1;
                    reports.push(report);
                }
            }
            Err(e) => {
                failed += 1;
                errors.push(format!("{}: {e}", p.name));
            }
        }
    }

    Ok(FinanceBatchResult {
        period: period.to_string(),
        total_partners: partners.len() as u32,
        analyzed,
        skipped,
        failed,
        errors,
        reports,
    })
}

/// CSV 批量导入指标并评估。表头需含 partner_name；可选 partner_type,period 及各指标列。
pub fn import_metrics_csv(conn: &Connection, csv_text: &str) -> Result<FinanceCsvImportResult, String> {
    ensure_schema(conn)?;
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(csv_text.as_bytes());
    let headers = rdr
        .headers()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    if headers.is_empty() {
        return Err("CSV 无表头".into());
    }

    let mut imported = 0u32;
    let mut failed = 0u32;
    let mut errors = Vec::new();
    let mut reports = Vec::new();
    let partners = db::list_partners(conn)?;

    for (i, row) in rdr.records().enumerate() {
        let rec = match row {
            Ok(r) => r,
            Err(e) => {
                failed += 1;
                errors.push(format!("行{}: {e}", i + 2));
                continue;
            }
        };
        let get = |key: &str| -> Option<String> {
            headers.iter().position(|h| h.eq_ignore_ascii_case(key)).and_then(|idx| {
                rec.get(idx).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
            })
        };
        let get_f = |key: &str| -> Option<f64> { get(key).and_then(|s| s.parse().ok()) };

        let name = match get("partner_name").or_else(|| get("name")).or_else(|| get("机构名称")) {
            Some(n) => n,
            None => {
                failed += 1;
                errors.push(format!("行{}: 缺少 partner_name", i + 2));
                continue;
            }
        };
        let matched = partners.iter().find(|p| p.name == name || p.alias == name);
        let partner_type = get("partner_type")
            .or_else(|| get("类型"))
            .or_else(|| matched.map(|p| p.partner_type.clone()))
            .unwrap_or_else(|| "loan".into());
        let period = get("period")
            .or_else(|| get("期间"))
            .unwrap_or_else(|| "2025全年".into());

        let mut guarantee = None;
        if get_f("leverage").is_some()
            || get_f("compensation_rate").is_some()
            || get("reserve_adequate").is_some()
        {
            guarantee = Some(GuaranteeSpecial {
                leverage: get_f("leverage"),
                compensation_rate: get_f("compensation_rate").or_else(|| get_f("compensationRate")),
                reserve_adequate: get("reserve_adequate").map(|s| {
                    s == "1" || s.eq_ignore_ascii_case("true") || s == "是"
                }),
                single_concentration: get_f("single_concentration")
                    .or_else(|| get_f("singleConcentration")),
                asset_ratio: get_f("asset_ratio").or_else(|| get_f("assetRatio")),
                fraud_signal: get("fraud_signal").map(|s| {
                    s == "1" || s.eq_ignore_ascii_case("true") || s == "是"
                }),
            });
        }

        let metrics = FinanceMetrics {
            revenue: get_f("revenue"),
            revenue_yoy: get_f("revenue_yoy").or_else(|| get_f("revenueYoy")),
            net_profit: get_f("net_profit").or_else(|| get_f("netProfit")),
            net_profit_yoy: get_f("net_profit_yoy").or_else(|| get_f("netProfitYoy")),
            net_margin: get_f("net_margin").or_else(|| get_f("netMargin")),
            gross_margin: get_f("gross_margin").or_else(|| get_f("grossMargin")),
            op_margin: get_f("op_margin").or_else(|| get_f("opMargin")),
            roa: get_f("roa"),
            roe: get_f("roe"),
            asset_liability_ratio: get_f("asset_liability_ratio")
                .or_else(|| get_f("assetLiabilityRatio")),
            current_ratio: get_f("current_ratio").or_else(|| get_f("currentRatio")),
            quick_ratio: get_f("quick_ratio").or_else(|| get_f("quickRatio")),
            equity_ratio: get_f("equity_ratio").or_else(|| get_f("equityRatio")),
            debt_ebitda: get_f("debt_ebitda").or_else(|| get_f("debtEbitda")),
            asset_turnover: get_f("asset_turnover").or_else(|| get_f("assetTurnover")),
            loan_originated: get_f("loan_originated").or_else(|| get_f("loanOriginated")),
            loan_balance: get_f("loan_balance").or_else(|| get_f("loanBalance")),
            npl_90: get_f("npl_90").or_else(|| get_f("npl90")),
            ocfo: get_f("ocfo"),
            fcf: get_f("fcf"),
            capex_ratio: get_f("capex_ratio").or_else(|| get_f("capexRatio")),
            dividend_payout: get_f("dividend_payout").or_else(|| get_f("dividendPayout")),
            guarantee,
            currency_unit: get("currency_unit")
                .or_else(|| get("currencyUnit"))
                .unwrap_or_else(|| "亿元人民币".into()),
            notes: get("notes").unwrap_or_else(|| format!("CSV 导入行 {}", i + 2)),
        };

        let req = FinanceAnalyzeRequest {
            partner_id: matched.map(|p| p.id.clone()),
            partner_name: name.clone(),
            partner_type,
            period,
            metrics: Some(metrics),
            use_demo: Some(false),
            document_text: None,
            source_kind: Some("csv".into()),
        };
        match analyze(&req) {
            Ok(mut report) => {
                let _ = enrich_report(conn, &mut report);
                match save_report(conn, &report) {
                    Ok(()) => {
                        imported += 1;
                        reports.push(report);
                    }
                    Err(e) => {
                        failed += 1;
                        errors.push(format!("{name}: {e}"));
                    }
                }
            }
            Err(e) => {
                failed += 1;
                errors.push(format!("{name}: {e}"));
            }
        }
    }

    Ok(FinanceCsvImportResult {
        imported,
        failed,
        errors,
        reports,
    })
}

/// 从财报文本用 LLM 抽取指标（可选）。
pub fn extract_metrics_from_text(
    cfg: &LlmConfig,
    document: &str,
) -> Result<crate::models::FinanceExtractResult, String> {
    if !cfg.is_ready() {
        return Err("LLM 未就绪，无法解析财报附件；请先在设置页配置大模型".into());
    }
    let system = r#"你是消费金融合作机构财务分析助手。从审计报告/年报/季报/附注文本中抽取机构信息与关键财务指标，只输出一个 JSON 对象（不要 markdown）。

字段：
{
  "partnerName":"企业全称",
  "period":"如 2025全年",
  "partnerType":"loan|guarantee|traffic|payment|data|collection|interbank|ops",
  "revenue":数字或null,
  "revenueYoy":同比%,
  "netProfit":,
  "netProfitYoy":,
  "netMargin":%,
  "grossMargin":,
  "opMargin":,
  "roa":,
  "roe":,
  "assetLiabilityRatio":%,
  "currentRatio":,
  "quickRatio":,
  "equityRatio":,
  "debtEbitda":,
  "assetTurnover":,
  "loanOriginated":,
  "loanBalance":,
  "npl90":,
  "ocfo":经营现金流净额,
  "fcf":,
  "capexRatio":%,
  "dividendPayout":%,
  "guarantee":{
    "leverage":担保放大倍数或null,
    "compensationRate":代偿率%或null,
    "reserveAdequate":准备金是否充足bool或null,
    "singleConcentration":%,
    "assetRatio":%,
    "fraudSignal":bool
  }|null,
  "currencyUnit":"亿元人民币",
  "notes":"审计意见/关键风险一句话"
}

规则：
1. 金额统一换算为「亿元人民币」（原文为元则 ÷1e8；万元 ÷1e4）。无法换算则填 null 并在 notes 说明。
2. 融资担保公司 partnerType 必须为 guarantee；担保费收入可写入 revenue；应收代偿、赔偿准备金、未到期责任准备金等写入 notes，并尽量估算 compensationRate / reserveAdequate。
3. 优先取「本期/期末」相对「上期/期初」计算同比%；利润为负也要如实填写。
4. 禁止编造原文没有的数字。"#;
    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: Some(system.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
        ChatMessage {
            role: "user".into(),
            content: Some(document.chars().take(24000).collect()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
    ];
    let reply = llm::chat_completion_timeout_retry(cfg, &messages, None, 150, 2)?;
    let content = reply.content.unwrap_or_default();
    let json_text = extract_json_object(&content)?;
    serde_json::from_str(&json_text).map_err(|e| format!("财务指标 JSON 解析失败: {e}"))
}

fn score_metrics(m: &FinanceMetrics, partner_type: &str) -> (Vec<DimensionScore>, Vec<String>) {
    let mut concerns = Vec::new();

    // 盈利
    let mut profit_score = 85i32;
    let mut profit_comment = String::from("盈利能力整体可接受");
    if let Some(yoy) = m.revenue_yoy {
        if yoy < -20.0 {
            profit_score -= 20;
            concerns.push(format!("营收同比下滑 {yoy:.1}%（>20% 关注信号）"));
        }
    }
    if let Some(nm) = m.net_margin {
        if nm >= 10.0 {
            profit_score += 5;
            profit_comment = format!("净利率 {nm:.2}% 表现优良");
        } else if nm < 3.0 {
            profit_score -= 15;
            concerns.push(format!("净利率偏低（{nm:.2}%）"));
        }
    }
    if let Some(npy) = m.net_profit_yoy {
        if npy > 30.0 {
            profit_score += 5;
        } else if npy < -30.0 {
            profit_score -= 10;
            concerns.push(format!("净利润同比大幅下滑 {npy:.1}%"));
        }
    }

    // 偿债
    let mut solvency_score = 85i32;
    let mut solvency_comment = String::from("偿债指标在可控区间");
    if let Some(al) = m.asset_liability_ratio {
        if al > 70.0 {
            solvency_score -= 25;
            concerns.push(format!("资产负债率 {al:.1}% 超过 70% 警戒线"));
            solvency_comment = "负债率偏高，需关注流动性与融资结构".into();
        } else if al < 55.0 {
            solvency_score += 5;
            solvency_comment = format!("资产负债率 {al:.1}% 良性");
        }
    }
    if let Some(cr) = m.current_ratio {
        if cr < 1.0 {
            solvency_score -= 20;
            concerns.push(format!("流动比率 {cr:.2} < 1.0 预警"));
        }
    }
    if let Some(qr) = m.quick_ratio {
        if qr < 0.8 {
            solvency_score -= 15;
            concerns.push(format!("速动比率 {qr:.2} < 0.8 预警"));
        }
    }
    if let Some(er) = m.equity_ratio {
        if er > 2.0 {
            solvency_score -= 15;
            concerns.push(format!("产权比率 {er:.2} > 2.0 预警"));
        }
    }
    if let Some(de) = m.debt_ebitda {
        if de > 5.0 {
            solvency_score -= 15;
            concerns.push(format!("负债/EBITDA {de:.2} > 5 预警"));
        }
    }

    // 运营
    let mut ops_score = 78i32;
    let mut ops_comment = String::from("运营效率稳定");
    if let Some(at) = m.asset_turnover {
        if at >= 0.8 {
            ops_score += 6;
            ops_comment = format!("资产周转率 {at:.2}，周转效率较好");
        } else if at < 0.3 {
            ops_score -= 8;
            concerns.push(format!("资产周转率 {at:.2} 偏低，关注运营效率"));
            ops_comment = format!("资产周转率 {at:.2} 偏弱");
        } else {
            ops_comment = format!("资产周转率 {at:.2}");
        }
    }
    if let Some(lb) = m.loan_balance {
        if let Some(lo) = m.loan_originated {
            if lo > 0.0 {
                ops_comment = format!(
                    "{ops_comment}；贷款发起 {lo:.0}、在贷 {lb:.0}（{}）",
                    m.currency_unit
                );
            }
        }
    }
    if m.npl_90.is_none() && (partner_type == "loan" || partner_type.contains("助贷")) {
        concerns.push("90天+不良率未披露（数据缺口）".into());
        ops_score -= 5;
    } else if let Some(npl) = m.npl_90 {
        if npl > 5.0 {
            ops_score -= 15;
            concerns.push(format!("90天+不良率 {npl:.2}% 偏高"));
        }
    }

    // 现金流
    let mut cash_score = 80i32;
    let mut cash_comment = String::from("现金流需结合明细核实");
    if let Some(fcf) = m.fcf {
        if fcf > 0.0 {
            cash_score += 8;
            cash_comment = format!("自由现金流为正（约 {fcf:.2}）");
            if let Some(np) = m.net_profit {
                if np > 0.0 {
                    let cover = fcf / np;
                    if cover >= 1.5 {
                        cash_score += 5;
                        cash_comment = format!("FCF/净利润覆盖约 {cover:.1} 倍，现金生成能力强");
                    }
                }
            }
        } else {
            cash_score -= 20;
            concerns.push("自由现金流为负或偏弱".into());
        }
    }
    if let Some(ocfo) = m.ocfo {
        if ocfo < 0.0 {
            cash_score -= 15;
            concerns.push("经营活动现金流为负".into());
        }
    }

    // 融担专项
    if partner_type == "guarantee" || partner_type.contains("担保") {
        if let Some(g) = &m.guarantee {
            if let Some(lev) = g.leverage {
                if lev > 10.0 {
                    concerns.push(format!("融担放大倍数 {lev:.1} 超红线（>10）→ 映射致命风险"));
                } else if lev > 8.0 {
                    concerns.push(format!("融担放大倍数 {lev:.1} > 8，橙色预警"));
                }
            }
            if let Some(cr) = g.compensation_rate {
                if cr > 5.0 {
                    concerns.push(format!("代偿率 {cr:.1}% > 5% 预警"));
                } else if cr > 3.0 {
                    concerns.push(format!("代偿率 {cr:.1}% > 3% 关注"));
                }
            }
            if g.reserve_adequate == Some(false) {
                concerns.push("准备金计提不足".into());
            }
            if let Some(c) = g.single_concentration {
                if c > 10.0 {
                    concerns.push(format!("单一客户集中度 {c:.1}% 超标"));
                } else if c > 8.0 {
                    concerns.push(format!("单一客户集中度 {c:.1}% 接近红线"));
                }
            }
            if let Some(ar) = g.asset_ratio {
                if ar < 60.0 {
                    concerns.push(format!("资产比例 {ar:.1}% < 60% 不合规"));
                } else if ar < 65.0 {
                    concerns.push(format!("资产比例 {ar:.1}% < 65% 关注"));
                }
            }
            if g.fraud_signal == Some(true) {
                concerns.push("存在财务造假/审计异常信号".into());
            }
        } else {
            concerns.push("融担专项：缺独立审计/台账，专项评级「待核实」".into());
        }
    }

    let scores = vec![
        DimensionScore {
            dimension: "盈利能力".into(),
            grade: grade_letter(profit_score),
            score: profit_score.clamp(0, 100),
            comment: profit_comment,
        },
        DimensionScore {
            dimension: "偿债能力".into(),
            grade: grade_letter(solvency_score),
            score: solvency_score.clamp(0, 100),
            comment: solvency_comment,
        },
        DimensionScore {
            dimension: "运营能力".into(),
            grade: grade_letter(ops_score),
            score: ops_score.clamp(0, 100),
            comment: ops_comment,
        },
        DimensionScore {
            dimension: "现金流能力".into(),
            grade: grade_letter(cash_score),
            score: cash_score.clamp(0, 100),
            comment: cash_comment,
        },
    ];

    (scores, concerns)
}

fn overall_rating(scores: &[DimensionScore], concerns: &[String]) -> String {
    let red_line = concerns.iter().any(|c| {
        c.contains("超红线") || c.contains("不合规") || c.contains("造假") || c.contains("致命")
    });
    if red_line {
        return "关注/预警".into();
    }
    let avg = if scores.is_empty() {
        70.0
    } else {
        scores.iter().map(|s| s.score as f64).sum::<f64>() / scores.len() as f64
    };
    if avg >= 88.0 {
        "A".into()
    } else if avg >= 82.0 {
        "A-".into()
    } else if avg >= 75.0 {
        "B+".into()
    } else if avg >= 68.0 {
        "B".into()
    } else {
        "C".into()
    }
}

fn grade_letter(score: i32) -> String {
    if score >= 88 {
        "A".into()
    } else if score >= 80 {
        "A-".into()
    } else if score >= 72 {
        "B+".into()
    } else if score >= 65 {
        "B".into()
    } else {
        "C".into()
    }
}

fn build_summary(
    name: &str,
    period: &str,
    rating: &str,
    risk_score: i32,
    concerns: &[String],
) -> String {
    if concerns.is_empty() {
        format!(
            "{name}（{period}）综合评级 {rating}，量化经营分 {risk_score}/100，关键财务指标未见明显警戒突破。"
        )
    } else {
        format!(
            "{name}（{period}）综合评级 {rating}，量化经营分 {risk_score}/100。关注点：{}",
            concerns.iter().take(4).cloned().collect::<Vec<_>>().join("；")
        )
    }
}

fn render_markdown(
    name: &str,
    ptype: &str,
    period: &str,
    m: &FinanceMetrics,
    scores: &[DimensionScore],
    overall: &str,
    risk_score: i32,
    concerns: &[String],
    related_flags: &[String],
    peers: Option<&[FinancePeerRow]>,
    series: Option<&[FinanceSeriesPoint]>,
    summary: &str,
    source_kind: &str,
) -> String {
    let mut md = String::new();
    md.push_str(&format!("# 季度经营评估报告 · {name}\n\n"));
    md.push_str(&format!(
        "- 机构类型：{ptype}\n- 报告期间：{period}\n- 综合评级：**{overall}**\n- 量化经营分：**{risk_score}/100**\n- 数据来源：{source_kind}\n- 币种单位：{}\n\n",
        m.currency_unit
    ));
    md.push_str(&format!("## 1. 摘要\n\n{summary}\n\n"));
    md.push_str("## 2. 总体财务快照表\n\n| 指标 | 数值 |\n|------|------|\n");
    push_row(&mut md, "总营收", m.revenue, &m.currency_unit);
    push_row_pct(&mut md, "营收同比", m.revenue_yoy);
    push_row(&mut md, "归母净利润", m.net_profit, &m.currency_unit);
    push_row_pct(&mut md, "净利润同比", m.net_profit_yoy);
    push_row_pct(&mut md, "净利率", m.net_margin);
    push_row_pct(&mut md, "毛利率", m.gross_margin);
    push_row_pct(&mut md, "ROA", m.roa);
    push_row_pct(&mut md, "ROE", m.roe);
    push_row_pct(&mut md, "资产负债率", m.asset_liability_ratio);
    push_row_num(&mut md, "流动比率", m.current_ratio);
    push_row_num(&mut md, "速动比率", m.quick_ratio);
    push_row_num(&mut md, "资产周转率", m.asset_turnover);
    push_row(&mut md, "经营现金流", m.ocfo, &m.currency_unit);
    push_row(&mut md, "自由现金流", m.fcf, &m.currency_unit);
    md.push('\n');

    md.push_str("## 3. 四维分析\n\n| 维度 | 评级 | 得分 | 判断 |\n|------|------|------|------|\n");
    for s in scores {
        md.push_str(&format!(
            "| {} | {} | {}/100 | {} |\n",
            s.dimension, s.grade, s.score, s.comment.replace('|', "/")
        ));
    }
    md.push('\n');

    if let Some(g) = &m.guarantee {
        md.push_str("## 4. 融担专项\n\n| 指标 | 数值 | 警戒参考 |\n|------|------|----------|\n");
        md.push_str(&format!(
            "| 放大倍数 | {} | ≤10（普通） |\n",
            g.leverage.map(|v| format!("{v:.2}")).unwrap_or_else(|| "—".into())
        ));
        md.push_str(&format!(
            "| 代偿率 | {} | <5% |\n",
            g.compensation_rate
                .map(|v| format!("{v:.2}%"))
                .unwrap_or_else(|| "—".into())
        ));
        md.push_str(&format!(
            "| 准备金足额 | {} | 应足额 |\n",
            match g.reserve_adequate {
                Some(true) => "是",
                Some(false) => "否",
                None => "—",
            }
        ));
        md.push_str(&format!(
            "| 单一客户集中度 | {} | ≤10% |\n",
            g.single_concentration
                .map(|v| format!("{v:.2}%"))
                .unwrap_or_else(|| "—".into())
        ));
        md.push_str(&format!(
            "| 资产比例 | {} | ≥60% |\n",
            g.asset_ratio
                .map(|v| format!("{v:.2}%"))
                .unwrap_or_else(|| "—".into())
        ));
        md.push('\n');
    }

    md.push_str("## 5. 同业横向对比\n\n");
    if let Some(peers) = peers {
        if peers.is_empty() {
            md.push_str("本期同类型机构暂无其他评估记录，导入更多机构后可自动排名。\n\n");
        } else {
            md.push_str("| 排名 | 机构 | 评级 | 经营分 |\n|------|------|------|--------|\n");
            for p in peers {
                md.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    p.rank, p.partner_name, p.overall_rating, p.risk_score
                ));
            }
            md.push('\n');
        }
    } else {
        md.push_str("（保存后自动汇总同业排名）\n\n");
    }

    md.push_str("## 6. 自身纵向时序\n\n");
    if let Some(series) = series {
        if series.len() < 2 {
            md.push_str("历史期次不足，继续按季度入库后可生成趋势对比。\n\n");
        } else {
            md.push_str("| 期间 | 经营分 | 评级 | 资产负债率 | 净利率 | FCF |\n|------|--------|------|------------|--------|-----|\n");
            for s in series {
                md.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} |\n",
                    s.period,
                    s.risk_score,
                    s.overall_rating,
                    s.asset_liability_ratio
                        .map(|v| format!("{v:.1}%"))
                        .unwrap_or_else(|| "—".into()),
                    s.net_margin
                        .map(|v| format!("{v:.1}%"))
                        .unwrap_or_else(|| "—".into()),
                    s.fcf.map(|v| format!("{v:.2}")).unwrap_or_else(|| "—".into()),
                ));
            }
            md.push('\n');
        }
    } else {
        md.push_str("（保存后自动汇总纵向对比）\n\n");
    }

    md.push_str("## 7. 股东/关联方穿透\n\n");
    if related_flags.is_empty() {
        md.push_str("- 暂无穿透提示\n\n");
    } else {
        for f in related_flags {
            md.push_str(&format!("- {f}\n"));
        }
        md.push('\n');
    }

    md.push_str("## 8. 关注点清单\n\n");
    if concerns.is_empty() {
        md.push_str("- 暂无警戒项\n");
    } else {
        for c in concerns {
            md.push_str(&format!("- {c}\n"));
        }
    }
    if !m.notes.is_empty() {
        md.push_str(&format!("\n## 附录 · 备注\n\n{}\n", m.notes));
    }
    md.push_str("\n---\n*智鉴风控官 · 经营守护自动生成；红线项须人工复核。*\n");
    md
}

fn push_row(md: &mut String, k: &str, v: Option<f64>, unit: &str) {
    match v {
        Some(n) => md.push_str(&format!("| {k} | {n:.2} {unit} |\n")),
        None => md.push_str(&format!("| {k} | — |\n")),
    }
}
fn push_row_pct(md: &mut String, k: &str, v: Option<f64>) {
    match v {
        Some(n) => md.push_str(&format!("| {k} | {n:.2}% |\n")),
        None => md.push_str(&format!("| {k} | — |\n")),
    }
}
fn push_row_num(md: &mut String, k: &str, v: Option<f64>) {
    match v {
        Some(n) => md.push_str(&format!("| {k} | {n:.2} |\n")),
        None => md.push_str(&format!("| {k} | — |\n")),
    }
}

fn extract_json_object(text: &str) -> Result<String, String> {
    let t = text.trim();
    if let Some(start) = t.find('{') {
        if let Some(end) = t.rfind('}') {
            return Ok(t[start..=end].to_string());
        }
    }
    Err("模型未返回 JSON 对象".into())
}

pub fn partner_type_label(code: &str) -> String {
    match code {
        "loan" => "助贷机构".into(),
        "guarantee" => "融资担保".into(),
        "traffic" => "流量引流".into(),
        "payment" => "支付机构".into(),
        "data" => "数据服务商".into(),
        "collection" => "催收机构".into(),
        "interbank" => "金市同业".into(),
        "ops" => "运营辅助".into(),
        other => other.to_string(),
    }
}

pub fn load_partner_meta(
    conn: &Connection,
    partner_id: &str,
) -> Result<(String, String, String), String> {
    let p = db::list_partners(conn)?
        .into_iter()
        .find(|x| x.id == partner_id)
        .ok_or_else(|| format!("机构不存在: {partner_id}"))?;
    Ok((p.name, p.partner_type, p.partner_type_label))
}
