use crate::db;
use crate::llm;
use crate::models::*;
use crate::partners;
use crate::pipeline;
use crate::rules;
use crate::state::AppState;
use tauri::State;
use uuid::Uuid;

fn with_db<T>(state: &AppState, f: impl FnOnce(&rusqlite::Connection) -> Result<T, String>) -> Result<T, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    f(&conn)
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "axiom-risk-agent".into(),
        version: "0.1.0".into(),
        product: "智联鉴控".into(),
    }
}

#[tauri::command]
pub fn list_partners(state: State<'_, AppState>) -> Result<Vec<Partner>, String> {
    with_db(&state, db::list_partners)
}

#[tauri::command]
pub fn import_partners_csv(
    state: State<'_, AppState>,
    csv: String,
    replace_all: Option<bool>,
) -> Result<PartnersDocumentImportResult, String> {
    let replace = replace_all.unwrap_or(true);
    with_db(&state, |conn| partners::import_from_csv(conn, &csv, replace))
}

#[tauri::command]
pub fn upsert_partner(
    state: State<'_, AppState>,
    partner: PartnerEditInput,
) -> Result<Vec<Partner>, String> {
    with_db(&state, |conn| partners::upsert_partner_edit(conn, &partner))
}

#[tauri::command]
pub fn import_partners_document(
    state: State<'_, AppState>,
    document: String,
    replace_all: Option<bool>,
) -> Result<PartnersDocumentImportResult, String> {
    let replace = replace_all.unwrap_or(true);
    with_db(&state, |conn| partners::import_from_csv(conn, &document, replace))
}

#[tauri::command]
pub fn list_rules(state: State<'_, AppState>) -> Result<Vec<RuleView>, String> {
    with_db(&state, rules::list_rule_views_from_db)
}

#[tauri::command]
pub fn apply_rule_overrides(
    state: State<'_, AppState>,
    overrides: Vec<RuleOverride>,
) -> Result<Vec<RuleView>, String> {
    with_db(&state, |conn| {
        for o in &overrides {
            db::set_override(conn, &o.rule_id, o.enabled, o.level)?;
        }
        db::insert_audit(
            conn,
            &AuditLog {
                id: Uuid::new_v4().to_string(),
                actor: "admin".into(),
                action: "apply_rule_overrides".into(),
                target: "rules".into(),
                detail: format!("count={}", overrides.len()),
                ts: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            },
        )?;
        Ok(())
    })?;
    list_rules(state)
}

#[tauri::command]
pub fn rule_chat(state: State<'_, AppState>, message: String) -> Result<RuleChatResult, String> {
    // Prefer LLM agent when configured; fallback to deterministic parser.
    let llm_cfg = with_db(&state, |conn| llm::load_config(conn))?;
    if llm_cfg.is_ready() {
        let agent_res = crate::agent::run_agent_with_state(
            &state,
            &llm_cfg,
            &AgentChatRequest {
                message: message.clone(),
                scope: Some("rules".into()),
                history: None,
            },
        )?;
        return Ok(RuleChatResult {
            ok: agent_res.ok,
            reply: if agent_res.used_llm {
                format!("{}{}", agent_res.reply, if agent_res.mutated { "（已通过 Agent 落库）" } else { "" })
            } else {
                agent_res.reply
            },
            actions: vec![],
        });
    }

    let (existing, custom_ids, parsed) = with_db(&state, |conn| {
        let views = rules::list_rule_views_from_db(conn)?;
        let customs = db::list_custom_rules(conn)?;
        let existing: Vec<(String, rules::RuleDef)> = views
            .iter()
            .map(|v| {
                (
                    v.partner_type.clone(),
                    rules::RuleDef {
                        id: v.id.clone(),
                        risk_point: v.risk_point.clone(),
                        level: v.level,
                        trigger: v.trigger.clone(),
                        data_source: v.data_source.clone(),
                        match_any_keywords: v.match_any_keywords.clone(),
                        metric: v.metric.clone(),
                        threshold: None,
                        legal_basis: v.legal_basis.clone(),
                        enabled: v.enabled,
                    },
                )
            })
            .collect();
        let custom_ids: Vec<String> = customs.iter().map(|(_, d)| d.id.clone()).collect();
        let parsed = rules::apply_rule_chat(&message, &existing, &custom_ids);
        Ok((existing, custom_ids, parsed))
    })?;
    let _ = (existing, custom_ids);

    if !parsed.ok || parsed.actions.is_empty() {
        let mut reply = parsed.reply;
        if !llm_cfg.enabled || llm_cfg.api_key.is_empty() {
            reply = format!(
                "{reply}\n\n提示：当前未启用 LLM。可在「设置」配置 OpenAI 兼容 API Key 后，用自然语言维护规则。"
            );
        }
        return Ok(RuleChatResult {
            ok: parsed.ok,
            reply,
            actions: parsed.actions,
        });
    }

    with_db(&state, |conn| {
        for action in &parsed.actions {
            match action {
                RuleChatAction::Override(o) => {
                    db::set_override(conn, &o.rule_id, o.enabled, o.level)?;
                }
                RuleChatAction::UpsertCustom(input) => {
                    let def = rules::custom_input_to_def(input);
                    db::upsert_custom_rule(conn, &input.partner_type, &def)?;
                }
                RuleChatAction::DeleteCustom { rule_id } => {
                    db::delete_custom_rule(conn, rule_id)?;
                }
            }
        }
        db::insert_audit(
            conn,
            &AuditLog {
                id: Uuid::new_v4().to_string(),
                actor: "admin".into(),
                action: "rule_chat".into(),
                target: "rules".into(),
                detail: message.clone(),
                ts: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            },
        )?;
        Ok(())
    })?;

    Ok(RuleChatResult {
        ok: true,
        reply: format!("{}（已落库）", parsed.reply),
        actions: parsed.actions,
    })
}

#[tauri::command]
pub fn agent_chat(
    state: State<'_, AppState>,
    req: AgentChatRequest,
) -> Result<AgentChatResponse, String> {
    let cfg = with_db(&state, |conn| llm::load_config(conn))?;
    if !cfg.is_ready() {
        return Ok(AgentChatResponse {
            ok: false,
            reply: "尚未配置 LLM。请打开「设置」，填写 API Base URL、Model 与 API Key 并启用。".into(),
            used_llm: false,
            mutated: false,
            tool_traces: vec![],
        });
    }
    crate::agent::run_agent_with_state(&state, &cfg, &req)
}

#[tauri::command]
pub fn upsert_rule(
    state: State<'_, AppState>,
    rule: RuleEditInput,
) -> Result<Vec<RuleView>, String> {
    with_db(&state, |conn| rules::upsert_rule_edit(conn, &rule))
}

#[tauri::command]
pub fn import_rules_document(
    state: State<'_, AppState>,
    document: String,
    replace_custom: Option<bool>,
) -> Result<RulesDocumentImportResult, String> {
    let replace = replace_custom.unwrap_or(true);
    with_db(&state, |conn| rules::import_from_csv(conn, &document, replace))
}

#[tauri::command]
pub fn get_llm_config(state: State<'_, AppState>) -> Result<LlmConfigPublic, String> {
    with_db(&state, |conn| {
        let cfg = llm::load_config(conn)?;
        llm::public_view(&cfg)
    })
}

#[tauri::command]
pub fn save_llm_config(
    state: State<'_, AppState>,
    config: LlmConfigSave,
) -> Result<LlmConfigPublic, String> {
    with_db(&state, |conn| {
        let view = llm::save_config(conn, &config)?;
        db::insert_audit(
            conn,
            &AuditLog {
                id: Uuid::new_v4().to_string(),
                actor: "admin".into(),
                action: "save_llm_config".into(),
                target: "llm".into(),
                detail: format!(
                    "enabled={} model={} has_key={}",
                    view.enabled, view.model, view.has_api_key
                ),
                ts: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            },
        )?;
        Ok(view)
    })
}

#[tauri::command]
pub fn test_llm_connection(state: State<'_, AppState>) -> Result<LlmTestResult, String> {
    let cfg = with_db(&state, |conn| llm::load_config(conn))?;
    Ok(llm::test_connection(&cfg))
}


#[tauri::command]
pub fn run_whistle_batch(
    state: State<'_, AppState>,
    live_heimao: Option<bool>,
    partner_ids: Option<Vec<String>>,
    trial: Option<bool>,
) -> Result<BatchResult, String> {
    let live = live_heimao.unwrap_or(false);
    let is_trial = trial.unwrap_or(false);
    let actor = if is_trial { "trial" } else { "user" };
    with_db(&state, |conn| {
        pipeline::run_whistle_batch_scoped(
            conn,
            live,
            actor,
            partner_ids.as_deref(),
            !is_trial,
            crate::tavily::SearchWindow::LastDay,
        )
    })
}

#[tauri::command]
pub fn get_whistle_schedule(state: State<'_, AppState>) -> Result<WhistleSchedule, String> {
    with_db(&state, crate::whistle_reports::load_schedule)
}

#[tauri::command]
pub fn save_whistle_schedule(
    state: State<'_, AppState>,
    schedule: WhistleScheduleSave,
) -> Result<WhistleSchedule, String> {
    with_db(&state, |conn| {
        let view = crate::whistle_reports::save_schedule(conn, &schedule)?;
        db::insert_audit(
            conn,
            &AuditLog {
                id: Uuid::new_v4().to_string(),
                actor: "user".into(),
                action: "whistle_schedule_save".into(),
                target: "app_settings".into(),
                detail: format!(
                    "daily={}@{} weekly={}@{}(dow={}) {}",
                    view.daily_enabled,
                    view.daily_time,
                    view.weekly_enabled,
                    view.weekly_time,
                    view.weekly_dow,
                    view.scope_hint
                ),
                ts: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            },
        )?;
        Ok(view)
    })
}

/// 立即执行日批/周批：跑监测并生成对应报告。kind = daily | weekly
/// 放到 blocking 线程，避免同步占用命令线程导致界面卡住、看不到「生成中」。
#[tauri::command]
pub async fn run_whistle_job(
    kind: String,
    partner_ids: Option<Vec<String>>,
) -> Result<WhistleJobResult, String> {
    let db_path = crate::db::default_db_path();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = crate::db::open(&db_path)?;
        crate::whistle_reports::run_job(&conn, &kind, "user", partner_ids)
    })
    .await
    .map_err(|e| format!("跑批任务异常: {e}"))?
}

#[tauri::command]
pub fn list_whistle_reports(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<WhistleReport>, String> {
    with_db(&state, |conn| {
        crate::whistle_reports::list_reports(conn, limit.unwrap_or(30))
    })
}

#[tauri::command]
pub fn get_tavily_settings(state: State<'_, AppState>) -> Result<TavilySettings, String> {
    with_db(&state, |conn| {
        let cfg = crate::tavily::load_config(conn)?;
        Ok(crate::tavily::to_public(&cfg))
    })
}

#[tauri::command]
pub fn save_tavily_settings(
    state: State<'_, AppState>,
    settings: TavilySettingsSave,
) -> Result<TavilySettings, String> {
    with_db(&state, |conn| {
        let view = crate::tavily::save_config(conn, &settings)?;
        db::insert_audit(
            conn,
            &AuditLog {
                id: Uuid::new_v4().to_string(),
                actor: "user".into(),
                action: "tavily_settings_save".into(),
                target: "app_settings".into(),
                detail: format!(
                    "enabled={} ready={} useInBatch={}",
                    view.enabled, view.ready, view.use_in_batch
                ),
                ts: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            },
        )?;
        Ok(view)
    })
}

#[tauri::command]
pub fn test_tavily(state: State<'_, AppState>) -> Result<TavilyTestResult, String> {
    let cfg = with_db(&state, crate::tavily::load_config)?;
    Ok(crate::tavily::test_connection(&cfg))
}

#[tauri::command]
pub fn get_enterprise_mcp_settings(
    state: State<'_, AppState>,
) -> Result<EnterpriseMcpSettings, String> {
    with_db(&state, |conn| {
        let cfg = crate::enterprise_mcp::load_config(conn)?;
        Ok(crate::enterprise_mcp::to_public(&cfg))
    })
}

#[tauri::command]
pub fn save_enterprise_mcp_settings(
    state: State<'_, AppState>,
    settings: EnterpriseMcpSettingsSave,
) -> Result<EnterpriseMcpSettings, String> {
    with_db(&state, |conn| {
        let view = crate::enterprise_mcp::save_config(conn, &settings)?;
        db::insert_audit(
            conn,
            &AuditLog {
                id: Uuid::new_v4().to_string(),
                actor: "user".into(),
                action: "save_enterprise_mcp_settings".into(),
                target: "enterprise_mcp".into(),
                detail: format!(
                    "qcc_enabled={} tyc_enabled={} qcc_key={} tyc_key={}",
                    view.qcc_enabled,
                    view.tyc_enabled,
                    view.qcc_has_api_key,
                    view.tyc_has_api_key
                ),
                ts: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            },
        )?;
        Ok(view)
    })
}

#[tauri::command]
pub fn test_enterprise_mcp(
    state: State<'_, AppState>,
    provider: String,
) -> Result<EnterpriseMcpTestResult, String> {
    let cfg = with_db(&state, crate::enterprise_mcp::load_config)?;
    Ok(crate::enterprise_mcp::test_provider(&cfg, provider.trim()))
}

#[tauri::command]
pub fn list_risk_clues(state: State<'_, AppState>) -> Result<Vec<RiskClue>, String> {
    with_db(&state, db::list_clues)
}

#[tauri::command]
pub fn update_clue(
    state: State<'_, AppState>,
    req: UpdateClueRequest,
) -> Result<RiskClue, String> {
    with_db(&state, |conn| {
        let before = db::list_clues(conn)?
            .into_iter()
            .find(|c| c.id == req.clue_id)
            .ok_or_else(|| format!("线索不存在: {}", req.clue_id))?;

        // 1/2 级不可静默销案：关闭须显式 status
        if before.level <= 2 {
            if let Some(st) = &req.status {
                let closed = st.contains("关闭") || st.contains("销案") || st == "closed";
                if closed && req.level.is_none() && req.actor.is_none() {
                    // still allow but require actor for audit — actor defaults below
                }
            }
        }

        let updated = db::update_clue(conn, &req.clue_id, req.level, req.status.clone())?;
        db::insert_audit(
            conn,
            &AuditLog {
                id: Uuid::new_v4().to_string(),
                actor: req.actor.unwrap_or_else(|| "user".into()),
                action: "update_clue".into(),
                target: req.clue_id.clone(),
                detail: format!(
                    "from_level={} to_level={:?} status={:?}",
                    before.level, req.level, req.status
                ),
                ts: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            },
        )?;
        Ok(updated)
    })
}

#[tauri::command]
pub async fn analyze_finance_report(
    state: State<'_, AppState>,
    req: FinanceAnalyzeRequest,
) -> Result<FinanceReport, String> {
    let mut req = req;
    if let Some(pid) = req.partner_id.clone().filter(|s| !s.is_empty()) {
        let (name, ptype, _) = with_db(&state, |conn| crate::finance::load_partner_meta(conn, &pid))?;
        if req.partner_name.trim().is_empty() {
            req.partner_name = name;
        }
        if req.partner_type.trim().is_empty() {
            req.partner_type = ptype;
        }
    }

    if req.metrics.is_none() && !req.use_demo.unwrap_or(false) {
        if let Some(doc) = req.document_text.clone().filter(|s| !s.trim().is_empty()) {
            let cfg = with_db(&state, |conn| llm::load_config(conn))?;
            let extracted = tauri::async_runtime::spawn_blocking(move || {
                crate::finance::extract_metrics_from_text(&cfg, &doc)
            })
            .await
            .map_err(|e| format!("财报解析任务异常: {e}"))??;
            if req.partner_name.trim().is_empty() {
                if let Some(n) = extracted
                    .partner_name
                    .as_ref()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                {
                    req.partner_name = n;
                }
            }
            if req.period.trim().is_empty() {
                if let Some(p) = extracted
                    .period
                    .as_ref()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                {
                    req.period = p;
                }
            }
            if let Some(pt) = extracted
                .partner_type
                .as_ref()
                .map(|s| s.trim().to_lowercase())
                .filter(|s| {
                    matches!(
                        s.as_str(),
                        "loan" | "guarantee" | "traffic" | "payment" | "data" | "collection"
                    )
                })
            {
                // 用户未选手动类型或仍为默认 loan 且文档识别为融担时，采用抽取结果
                if req.partner_type.trim().is_empty()
                    || (req.partner_id.as_ref().map(|s| s.is_empty()).unwrap_or(true)
                        && req.partner_type == "loan"
                        && pt == "guarantee")
                {
                    req.partner_type = pt;
                }
            }
            req.metrics = Some(extracted.metrics);
        }
    }
    if req.partner_name.trim().is_empty() {
        return Err("未能识别机构名称，请手动填写后再分析".into());
    }
    if req.period.trim().is_empty() {
        req.period = "报告期".into();
    }

    let mut report = crate::finance::analyze(&req)?;
    with_db(&state, |conn| {
        let _ = crate::finance::enrich_report(conn, &mut report);
        crate::finance::save_report(conn, &report)?;
        db::insert_audit(
            conn,
            &AuditLog {
                id: Uuid::new_v4().to_string(),
                actor: "user".into(),
                action: "finance_analyze".into(),
                target: report.id.clone(),
                detail: format!(
                    "partner={} period={} rating={} score={}",
                    report.partner_name, report.period, report.overall_rating, report.risk_score
                ),
                ts: report.created_at.clone(),
            },
        )?;
        Ok(report)
    })
}

#[tauri::command]
pub fn list_finance_reports(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<FinanceReport>, String> {
    with_db(&state, |conn| {
        crate::finance::list_reports(conn, limit.unwrap_or(30))
    })
}

#[tauri::command]
pub fn compare_finance_peers(
    state: State<'_, AppState>,
    partner_type: String,
    period: String,
    limit: Option<u32>,
) -> Result<Vec<FinancePeerRow>, String> {
    with_db(&state, |conn| {
        crate::finance::compare_peers(conn, &partner_type, &period, limit.unwrap_or(20))
    })
}

#[tauri::command]
pub fn list_finance_series(
    state: State<'_, AppState>,
    partner_id: Option<String>,
    partner_name: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<FinanceSeriesPoint>, String> {
    with_db(&state, |conn| {
        crate::finance::list_series(
            conn,
            partner_id.as_deref().unwrap_or(""),
            partner_name.as_deref().unwrap_or(""),
            limit.unwrap_or(12),
        )
    })
}

#[tauri::command]
pub fn run_finance_quarter_batch(
    state: State<'_, AppState>,
    period: String,
    fill_demo_if_empty: Option<bool>,
) -> Result<FinanceBatchResult, String> {
    with_db(&state, |conn| {
        let res = crate::finance::run_quarter_batch(
            conn,
            &period,
            fill_demo_if_empty.unwrap_or(false),
        )?;
        db::insert_audit(
            conn,
            &AuditLog {
                id: Uuid::new_v4().to_string(),
                actor: "user".into(),
                action: "finance_quarter_batch".into(),
                target: period.clone(),
                detail: format!(
                    "total={} analyzed={} skipped={} failed={}",
                    res.total_partners, res.analyzed, res.skipped, res.failed
                ),
                ts: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            },
        )?;
        Ok(res)
    })
}

#[tauri::command]
pub fn import_finance_csv(
    state: State<'_, AppState>,
    csv: String,
) -> Result<FinanceCsvImportResult, String> {
    with_db(&state, |conn| {
        let res = crate::finance::import_metrics_csv(conn, &csv)?;
        db::insert_audit(
            conn,
            &AuditLog {
                id: Uuid::new_v4().to_string(),
                actor: "user".into(),
                action: "finance_csv_import".into(),
                target: "finance_reports".into(),
                detail: format!("imported={} failed={}", res.imported, res.failed),
                ts: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            },
        )?;
        Ok(res)
    })
}

#[tauri::command]
pub fn get_finance_demo_metrics() -> FinanceMetrics {
    crate::finance::demo_lexin_metrics()
}

#[tauri::command]
pub async fn run_admission_review(
    _state: State<'_, AppState>,
    req: AdmissionReviewRequest,
) -> Result<AdmissionReview, String> {
    // 独立开库连接 + blocking 线程，避免长时间占用 AppState 锁拖死 UI
    let db_path = crate::db::default_db_path();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = crate::db::open(&db_path)?;
        crate::admission::run_review(&conn, &req)
    })
    .await
    .map_err(|e| format!("准入研判任务异常: {e}"))?
}

#[tauri::command]
pub fn list_admission_reviews(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<AdmissionReview>, String> {
    with_db(&state, |conn| {
        crate::admission::list_reviews(conn, limit.unwrap_or(30))
    })
}

#[tauri::command]
pub fn import_admission_knowledge(
    state: State<'_, AppState>,
    req: AdmissionKnowledgeImportRequest,
) -> Result<AdmissionKnowledge, String> {
    with_db(&state, |conn| crate::admission::import_knowledge(conn, &req))
}

#[tauri::command]
pub fn list_admission_knowledge(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<AdmissionKnowledge>, String> {
    with_db(&state, |conn| {
        crate::admission::list_knowledge(conn, limit.unwrap_or(40))
    })
}

#[tauri::command]
pub fn ingest_admission_case(
    state: State<'_, AppState>,
    req: AdmissionCaseIngestRequest,
) -> Result<AdmissionCaseIngestResult, String> {
    with_db(&state, |conn| crate::admission::ingest_case(conn, &req))
}

#[tauri::command]
pub fn list_audit_logs(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<AuditLog>, String> {
    with_db(&state, |conn| db::list_audit(conn, limit.unwrap_or(50)))
}

#[tauri::command]
pub fn get_dashboard_snapshot(state: State<'_, AppState>) -> Result<DashboardSnapshot, String> {
    with_db(&state, |conn| {
        let clues = db::list_clues(conn)?;
        let partners = db::list_partners(conn)?;
        Ok(build_dashboard(&partners, &clues))
    })
}

fn build_dashboard(partners: &[Partner], clues: &[RiskClue]) -> DashboardSnapshot {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let kpi_level1 = clues.iter().filter(|c| c.level == 1).count() as u32;
    let kpi_level2 = clues.iter().filter(|c| c.level == 2).count() as u32;
    let kpi_pending = clues
        .iter()
        .filter(|c| c.status.contains("待") || c.status.contains("复核"))
        .count() as u32;
    let kpi_effective = clues
        .iter()
        .filter(|c| c.denoise_status != "repeat")
        .count() as u32;

    let health = if clues.is_empty() {
        100.0
    } else {
        let penalty = (kpi_level1 as f64) * 12.0 + (kpi_level2 as f64) * 5.0;
        (100.0 - penalty).clamp(0.0, 100.0)
    };

    let type_meta = [
        ("loan", "助贷机构", "投诉 · 约谈 · 现金流", "violet"),
        ("guarantee", "融资担保", "罚单 · 解约 · 股权", "blue"),
        ("traffic", "流量引流", "合作稳定 · 盈利", "amber"),
        ("payment", "支付机构", "政策 · 处罚", "rose"),
        ("data", "数据服务商", "资质 · 政策前瞻", "sky"),
        ("collection", "催收机构", "暴力投诉 · 涉诉", "orange"),
    ];

    let type_cards = type_meta
        .iter()
        .map(|(key, title, subtitle, tone)| {
            let label = title;
            let yesterday_new = clues
                .iter()
                .filter(|c| c.partner_type == *label || c.partner_type.contains(&key[0..2]))
                .filter(|c| {
                    PartnerType::parse(
                        partners
                            .iter()
                            .find(|p| p.name == c.partner)
                            .map(|p| p.partner_type.as_str())
                            .unwrap_or(""),
                    )
                    .map(|t| t.as_str() == *key)
                    .unwrap_or(c.partner_type == *label)
                })
                .count() as u32;
            // simpler count by partner_type label
            let count = clues.iter().filter(|c| c.partner_type == *label).count() as u32;
            let high = clues
                .iter()
                .filter(|c| c.partner_type == *label && c.level <= 2)
                .count() as u32;
            let total_partners = partners.iter().filter(|p| p.partner_type == *key).count() as u32;
            TypeCard {
                key: (*key).into(),
                title: (*title).into(),
                subtitle: (*subtitle).into(),
                yesterday_new: if count > 0 { count } else { yesterday_new },
                high_count: high,
                progress: if total_partners == 0 {
                    0
                } else {
                    ((total_partners.saturating_sub(high)) * 100 / total_partners.max(1)).min(100)
                },
                tone: (*tone).into(),
            }
        })
        .collect();

    let events: Vec<EventItem> = clues
        .iter()
        .filter(|c| c.level <= 2)
        .take(6)
        .map(|c| EventItem {
            title: c.title.clone(),
            time: c.event_date.clone(),
            level: c.level,
        })
        .collect();

    DashboardSnapshot {
        user_name: "风控同学".into(),
        monitor_date: today,
        kpi_effective,
        kpi_level1,
        kpi_level2,
        kpi_pending,
        health_score: (health * 10.0).round() / 10.0,
        health_rank_label: "组合风险健康分".into(),
        type_cards,
        trend: vec![
            TrendPoint { day: "周一".into(), value: 0, highlight: false },
            TrendPoint { day: "周二".into(), value: 0, highlight: false },
            TrendPoint { day: "周三".into(), value: 0, highlight: false },
            TrendPoint { day: "周四".into(), value: 0, highlight: false },
            TrendPoint { day: "周五".into(), value: 0, highlight: false },
            TrendPoint { day: "周六".into(), value: 0, highlight: false },
            TrendPoint { day: "周日".into(), value: kpi_effective, highlight: true },
        ],
        events,
    }
}
