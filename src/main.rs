#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod file_index;
mod fonts;
mod settings;
mod tray;
mod watcher;

fn main() -> eframe::Result<()> {
    let settings = settings::load();
    let (tray_tx, tray_rx) = std::sync::mpsc::channel();
    tray::run_tray_thread(
        tray_tx.clone(),
        settings.locale_zh,
        settings.always_visible,
    );
    app::run(tray_rx)
}
