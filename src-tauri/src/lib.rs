mod automation;
mod dashboard;
mod frpc;
mod ports;
mod store;
mod transfer;
mod tray;

use frpc::Runtime;
use store::Store;
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

pub use automation::run_cli;

struct AutomationTask(tauri::async_runtime::JoinHandle<()>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    // 单实例：必须最先注册。再次打开应用时不新开进程，而是唤起已有窗口
    // （否则两个实例会各自起一套 frpc，抢同一个 remotePort）。
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        tray::show_main(app);
    }));

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            #[cfg(desktop)]
            {
                app.handle()
                    .plugin(tauri_plugin_updater::Builder::new().build())?;
                app.handle().plugin(tauri_plugin_process::init())?;
            }
            let s = Store::load(&app.handle()).expect("failed to load store");
            app.manage(s);
            app.manage(Runtime::new());
            app.manage(dashboard::DashboardClient::new()?);
            tray::setup(&app.handle())?;
            // 恢复上次的运行状态：store 里 enabled=true 的 proxy 自动拉起 frpc，
            // 保证界面上的「已启用」与真实进程一致。
            frpc::restore_on_startup(app.handle().clone());
            app.manage(AutomationTask(automation::start(app.handle().clone())));
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // 关闭窗口 = 隐藏到后台（托盘），frpc 继续在后台跑。
                // 真正退出走托盘右键菜单的「退出」。
                api.prevent_close();
                let _ = window.hide();
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
            dashboard::dashboard_overview,
            frpc::list_runtime,
            frpc::start_proxy,
            frpc::stop_proxy,
            frpc::start_server,
            frpc::stop_server,
            frpc::server_logs,
            frpc::check_frpc,
            frpc::stop_all_proxies,
            transfer::export_config,
            transfer::import_config,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app.state::<AutomationTask>().0.abort();
            }
        });
}
