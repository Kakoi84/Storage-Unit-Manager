use crate::{app::AppContext, models::ReportSummary};

use eframe::egui;

#[derive(Default)]
pub struct ReportsPage {
    summary: Option<ReportSummary>,
    loaded: bool,
    data_revision_seen: u64,
    error_message: Option<String>,
}

impl ReportsPage {
    pub fn show(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        if !self.loaded || self.data_revision_seen != context.data_revision() {
            self.refresh(context);
        }

        ui.horizontal(|ui| {
            ui.heading("📈 Reports");

            if ui.button("Refresh").clicked() {
                self.refresh(context);

                if self.error_message.is_none() {
                    context.set_status_message("Reports refreshed");
                }
            }
        });

        ui.separator();

        if let Some(error_message) = &self.error_message {
            ui.colored_label(egui::Color32::RED, error_message);

            return;
        }

        let Some(summary) = &self.summary else {
            ui.label("Report information is unavailable.");

            return;
        };

        self.show_occupancy_section(ui, summary);

        ui.add_space(18.0);
        ui.separator();
        ui.add_space(10.0);

        self.show_operations_section(ui, summary);

        ui.add_space(18.0);
        ui.separator();
        ui.add_space(10.0);

        self.show_financial_section(ui, summary);
    }

    fn show_occupancy_section(&self, ui: &mut egui::Ui, summary: &ReportSummary) {
        ui.heading("Occupancy");

        ui.add_space(8.0);

        egui::Grid::new("reports_occupancy_cards")
            .num_columns(3)
            .spacing([18.0, 10.0])
            .show(ui, |ui| {
                metric_card(ui, "Total Units", summary.total_units.to_string());

                metric_card(ui, "Occupied", summary.occupied_units.to_string());

                metric_card(ui, "Vacant", summary.vacant_units.to_string());

                ui.end_row();
            });

        ui.add_space(12.0);

        let occupancy_percent = summary.occupancy_percent();

        ui.label(format!("Occupancy Rate: {:.1}%", occupancy_percent,));

        ui.add(
            egui::ProgressBar::new((occupancy_percent / 100.0).clamp(0.0, 1.0) as f32)
                .show_percentage()
                .desired_width(420.0),
        );
    }

    fn show_operations_section(&self, ui: &mut egui::Ui, summary: &ReportSummary) {
        ui.heading("Operations");

        ui.add_space(8.0);

        egui::Grid::new("reports_operations_cards")
            .num_columns(2)
            .spacing([18.0, 10.0])
            .show(ui, |ui| {
                metric_card(ui, "Active Tenants", summary.active_tenants.to_string());

                metric_card(ui, "Active Rentals", summary.active_rentals.to_string());

                ui.end_row();
            });

        ui.add_space(8.0);

        if summary.active_rentals == summary.occupied_units {
            ui.label("🟢 Active rentals match occupied units.");
        } else {
            ui.colored_label(
                egui::Color32::RED,
                format!(
                    "🔴 Data mismatch: {} occupied units but {} active rentals.",
                    summary.occupied_units, summary.active_rentals,
                ),
            );
        }
    }

    fn show_financial_section(&self, ui: &mut egui::Ui, summary: &ReportSummary) {
        ui.heading("Financial Summary");

        ui.add_space(8.0);

        egui::Grid::new("reports_financial_cards")
            .num_columns(2)
            .spacing([18.0, 10.0])
            .show(ui, |ui| {
                metric_card(
                    ui,
                    "Current Monthly Rent",
                    format_money(summary.monthly_rent_cents),
                );

                metric_card(
                    ui,
                    "Total Payments Received",
                    format_money(summary.total_payments_cents),
                );

                ui.end_row();
            });

        ui.add_space(10.0);

        ui.label("Current Monthly Rent is based on active rental agreements.");

        ui.label("Total Payments Received includes all recorded payment history.");
    }

    fn refresh(&mut self, context: &mut AppContext) {
        match context.database().get_report_summary() {
            Ok(summary) => {
                self.summary = Some(summary);
                self.loaded = true;

                self.data_revision_seen = context.data_revision();

                self.error_message = None;
            }

            Err(error) => {
                self.summary = None;
                self.loaded = true;

                let message = format!("Unable to load report summary: {}", error,);

                context.set_status_message(message.clone());

                self.error_message = Some(message);
            }
        }
    }
}

fn metric_card(ui: &mut egui::Ui, title: &str, value: String) {
    ui.group(|ui| {
        ui.set_min_width(190.0);

        ui.label(egui::RichText::new(title).small());

        ui.add_space(4.0);

        ui.label(egui::RichText::new(value).heading().strong());
    });
}

fn format_money(cents: i64) -> String {
    format!("${}.{:02}", cents / 100, cents.abs() % 100,)
}
