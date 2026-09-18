use crate::{config::Config, page::Page};
use eframe::egui;

pub fn show(ui: &mut egui::Ui, current_page: &mut Page, config: &Config) {
    ui.heading(&config.company_name);
    ui.label(format!("Version {}", config.version));
    ui.separator();

    navigation_button(ui, current_page, Page::Dashboard, "🏠 Dashboard");

    navigation_button(ui, current_page, Page::Units, "📦 Units");

    navigation_button(ui, current_page, Page::Tenants, "👤 Tenants");

    navigation_button(ui, current_page, Page::Rentals, "🔑 Rentals");

    navigation_button(ui, current_page, Page::Payments, "💲 Payments");

    navigation_button(ui, current_page, Page::Padlocks, "🔒 Padlocks");

    navigation_button(ui, current_page, Page::Reports, "📈 Reports");

    navigation_button(ui, current_page, Page::Settings, "⚙ Settings");
}

fn navigation_button(ui: &mut egui::Ui, current_page: &mut Page, page: Page, text: &str) {
    let selected = *current_page == page;

    if ui.selectable_label(selected, text).clicked() {
        *current_page = page;
    }
}
