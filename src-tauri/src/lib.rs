mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_dashboard_snapshot,
            commands::list_risk_clues,
            commands::app_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
