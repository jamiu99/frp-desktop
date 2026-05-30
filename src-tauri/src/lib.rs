mod frpc;
mod ports;
mod store;
mod tray;

use frpc::Runtime;
use store::Store;
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let s = Store::load(&app.handle())
                .expect("failed to load store");
            app.manage(s);
            app.manage(Runtime::new());
            tray::setup(&app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let close_to_tray = app
                    .try_state::<Store>()
                    .map(|s| s.snapshot().settings.close_to_tray)
                    .unwrap_or(false);

                if close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    if let Some(rt) = app.try_state::<Runtime>() {
                        frpc::shutdown_all(&rt);
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            ports::list_ports,
            store::get_state,
            store::create_server,
            store::update_server,
            store::delete_server,
            store::create_proxy,
            store::update_proxy,
            store::delete_proxy,
            store::set_proxy_enabled,
            store::update_settings,
            frpc::list_runtime,
            frpc::start_proxy,
            frpc::stop_proxy,
            frpc::start_server,
            frpc::stop_server,
            frpc::server_logs,
            frpc::check_frpc,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
