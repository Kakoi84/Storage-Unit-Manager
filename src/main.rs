mod app;
mod config;
mod database;
mod models;
mod page;
mod pages;
mod widgets;

use app::StorageManager;

fn main() -> eframe::Result<()> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("Storage Manager"),
        ..Default::default()
    };

    eframe::run_native(
        "Storage Manager",
        options,
        Box::new(|cc| Ok(Box::new(StorageManager::new(cc)))),
    )
}
