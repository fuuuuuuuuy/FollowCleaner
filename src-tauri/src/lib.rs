mod db;
mod error;
mod features;
mod models;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 日志输出到标准输出（tauri dev 控制台可见）；默认 info 级别，RUST_LOG 可覆盖
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("无法获取应用数据目录");
            let conn =
                db::open_and_migrate(&data_dir).expect("数据库初始化失败");
            app.manage(db::DbState(std::sync::Mutex::new(conn)));
            log::info!("FollowCleaner core ready, data dir: {}", data_dir.display());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 账号
            features::accounts::query_accounts,
            features::accounts::overview_counts,
            features::accounts::assign_categories,
            features::accounts::remove_category_from_accounts,
            features::accounts::update_note,
            // 分类
            features::categories::list_categories,
            features::categories::category_counts,
            features::categories::create_category,
            features::categories::rename_category,
            features::categories::delete_category,
            features::categories::move_category,
            // 导入 / 导出
            features::importx::parse_import_file,
            features::importx::commit_import,
            features::importx::write_template,
            features::importx::export_accounts,
            // 平台适配器
            features::adapters::bilibili::bilibili_qr_generate,
            features::adapters::bilibili::bilibili_qr_poll,
            features::adapters::bilibili::bilibili_status,
            features::adapters::bilibili::bilibili_logout,
            features::adapters::bilibili::bilibili_fetch_follows,
            features::adapters::bilibili::bilibili_unfollow_batch,
        ])
        .run(tauri::generate_context!())
        .expect("FollowCleaner 启动失败");
}
