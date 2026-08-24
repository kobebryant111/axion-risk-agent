mod admission;
mod agent;
mod commands;
mod db;
mod enterprise_mcp;
mod finance;
mod llm;
mod mcp_http;
mod models;
mod partners;
mod pipeline;
mod rules;
mod sources;
mod state;
mod tavily;
mod webview_runtime;
mod whistle_ai;
mod whistle_reports;

use state::AppState;
use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    webview_runtime::ensure_or_install();
    let db_path = db::default_db_path();
    let conn = db::open(&db_path).expect("failed to open sqlite database");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            conn: Mutex::new(conn),
        })
        .setup(|app| {
            whistle_reports::spawn_scheduler(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::get_dashboard_snapshot,
            commands::list_risk_clues,
            commands::list_partners,
            commands::import_partners_csv,
            commands::upsert_partner,
            commands::import_partners_document,
            commands::list_rules,
            commands::apply_rule_overrides,
            commands::rule_chat,
            commands::agent_chat,
            commands::upsert_rule,
            commands::import_rules_document,
            commands::get_llm_config,
            commands::save_llm_config,
            commands::test_llm_connection,
            commands::run_whistle_batch,
            commands::get_whistle_schedule,
            commands::save_whistle_schedule,
            commands::run_whistle_job,
            commands::list_whistle_reports,
            commands::get_tavily_settings,
            commands::save_tavily_settings,
            commands::test_tavily,
            commands::get_enterprise_mcp_settings,
            commands::save_enterprise_mcp_settings,
            commands::test_enterprise_mcp,
            commands::analyze_finance_report,
            commands::list_finance_reports,
            commands::compare_finance_peers,
            commands::list_finance_series,
            commands::run_finance_quarter_batch,
            commands::import_finance_csv,
            commands::get_finance_demo_metrics,
            commands::run_admission_review,
            commands::list_admission_reviews,
            commands::import_admission_knowledge,
            commands::list_admission_knowledge,
            commands::ingest_admission_case,
            commands::update_clue,
            commands::list_audit_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
