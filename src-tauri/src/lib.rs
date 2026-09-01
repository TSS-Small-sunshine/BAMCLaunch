mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::version::fetch_version_manifest,
            commands::download::download_version_json,
            commands::download::download_version_jar,
            commands::download::download_version_assets,
            commands::download::download_version_libraries,
            commands::java::scan_java_installations,
            commands::launch::launch_version,
            commands::settings::load_settings,
            commands::settings::save_settings,
            commands::instances::list_instances,
            commands::instances::kill_running_instance,
            commands::accounts::list_accounts,
            commands::accounts::add_offline_account,
            commands::accounts::remove_account,
            commands::accounts::set_active_account
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
