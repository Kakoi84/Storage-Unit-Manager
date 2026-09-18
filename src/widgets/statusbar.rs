use crate::{
    config::Config,
    database::Database,
};

use eframe::egui;
use std::time::Duration;

pub fn show(
    ui: &mut egui::Ui,
    config: &Config,
    status_message: &str,
    database: &Database,
) {
    let database_connected =
    database.health_check().is_ok();

    let spinner_frames = ["|", "/", "-", "\\"];

    let time = ui.ctx().input(|input| input.time);

    let frame =
    ((time * 8.0) as usize) % spinner_frames.len();

    ui.horizontal(|ui| {
        ui.label(status_message);

        ui.with_layout(
            egui::Layout::right_to_left(
                egui::Align::Center,
            ),
            |ui| {
                ui.label(format!(
                    "Version {}",
                    config.version,
                ));

                ui.separator();

                if database_connected {
                    ui.colored_label(
                        egui::Color32::GREEN,
                        "Connected",
                    );

                    ui.monospace(
                        spinner_frames[frame],
                    );

                    ui.label("Database Status:");

                    ui.ctx().request_repaint_after(
                        Duration::from_millis(125),
                    );
                } else {
                    ui.colored_label(
                        egui::Color32::RED,
                        "Disconnected",
                    );

                    ui.monospace("X");

                    ui.label("Database Status:");
                }
            },
        );
    });
}
