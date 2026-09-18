use crate::{
    app::AppContext,
    models::{Padlock, ReportSummary, Unit, UnitSize},
};

use eframe::egui;
use std::collections::{BTreeMap, HashMap};

#[derive(Default)]
pub struct DashboardPage {
    summary: Option<ReportSummary>,
    units: Vec<Unit>,
    padlocks: Vec<Padlock>,
    loaded: bool,
    data_revision_seen: u64,
    error_message: Option<String>,
}

impl DashboardPage {
    pub fn show(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        if !self.loaded || self.data_revision_seen != context.data_revision() {
            self.refresh(context);
        }

        ui.horizontal(|ui| {
            ui.heading("🏠 Dashboard");

            if ui.button("Refresh").clicked() {
                self.refresh(context);

                if self.error_message.is_none() {
                    context.set_status_message("Dashboard refreshed");
                }
            }
        });

        ui.label(egui::RichText::new(context.config().company_name.as_str()).strong());

        ui.separator();

        if let Some(error_message) = &self.error_message {
            ui.colored_label(egui::Color32::RED, error_message);

            return;
        }

        let Some(summary) = &self.summary else {
            ui.label("Dashboard information is unavailable.");

            return;
        };

        self.show_summary_cards(ui, summary);

        ui.add_space(18.0);
        ui.separator();
        ui.add_space(10.0);

        self.show_unit_layout(ui);
    }

    fn show_summary_cards(&self, ui: &mut egui::Ui, summary: &ReportSummary) {
        ui.heading("Property Summary");
        ui.add_space(8.0);

        egui::Grid::new("dashboard_summary_cards")
            .num_columns(4)
            .spacing([18.0, 10.0])
            .show(ui, |ui| {
                metric_card(ui, "Total Units", summary.total_units.to_string());

                metric_card(ui, "Occupied", summary.occupied_units.to_string());

                metric_card(ui, "Vacant", summary.vacant_units.to_string());

                metric_card(
                    ui,
                    "Needs Cleaned",
                    summary.needs_cleaned_units.to_string(),
                );

                ui.end_row();

                metric_card(ui, "Active Tenants", summary.active_tenants.to_string());

                metric_card(ui, "Active Rentals", summary.active_rentals.to_string());

                metric_card(ui, "Monthly Rent", format_money(summary.monthly_rent_cents));

                metric_card(
                    ui,
                    "Payments Received",
                    format_money(summary.total_payments_cents),
                );

                ui.end_row();
            });

        ui.add_space(10.0);

        let occupancy = summary.occupancy_percent();

        ui.label(format!("Occupancy Rate: {:.1}%", occupancy,));

        ui.add(
            egui::ProgressBar::new((occupancy / 100.0).clamp(0.0, 1.0) as f32)
                .show_percentage()
                .desired_width(420.0),
        );
    }

    fn show_unit_layout(&self, ui: &mut egui::Ui) {
        let padlocks_by_unit: HashMap<i64, &Padlock> = self
            .padlocks
            .iter()
            .filter_map(|padlock| padlock.unit_id.map(|unit_id| (unit_id, padlock)))
            .collect();

        let occupied_without_lock = self
            .units
            .iter()
            .filter(|unit| {
                unit.occupied
                    && !unit.red_lock
                    && !padlocks_by_unit.contains_key(&unit.id)
            })
            .count();

        ui.horizontal_wrapped(|ui| {
            ui.heading("Storage Unit Layout");

            ui.separator();

            ui.colored_label(vacant_color(), "■ Vacant");

            ui.colored_label(occupied_color(), "■ Occupied");

            ui.colored_label(needs_cleaned_color(), "■ Needs Cleaned");

            ui.label("🟨 Yellow Lock");
            ui.label("🟥 Red Lock");
            ui.label("🔒 Tracked Padlock");

            if occupied_without_lock > 0 {
                ui.colored_label(
                    warning_color(),
                    format!(
                        "⚠ {} occupied without customer lock",
                        occupied_without_lock,
                    ),
                );
            }
        });

        ui.add_space(8.0);

        if self.units.is_empty() {
            ui.label("No storage units have been added.");

            return;
        }

        let mut buildings: BTreeMap<String, BTreeMap<String, Vec<&Unit>>> = BTreeMap::new();

        for unit in &self.units {
            let building = if unit.building.trim().is_empty() {
                "Unassigned Building".to_string()
            } else {
                unit.building.trim().to_string()
            };

            let section = if unit.layout_section.trim().is_empty() {
                "Main Section".to_string()
            } else {
                unit.layout_section.trim().to_string()
            };

            buildings
                .entry(building)
                .or_default()
                .entry(section)
                .or_default()
                .push(unit);
        }

        for sections in buildings.values_mut() {
            for units in sections.values_mut() {
                units.sort_by(|left, right| {
                    left.display_order
                        .cmp(&right.display_order)
                        .then_with(|| {
                            left.unit_number
                                .to_lowercase()
                                .cmp(&right.unit_number.to_lowercase())
                        })
                });
            }
        }

        let available_height = ui.available_height().max(200.0);

        egui::ScrollArea::vertical()
            .id_salt("dashboard_unit_layout_scroll")
            .auto_shrink([false, false])
            .max_height(available_height)
            .show(ui, |ui| {
                for (building, sections) in buildings {
                    ui.add_space(10.0);

                    ui.label(egui::RichText::new(&building).strong().size(17.0));

                    ui.add_space(6.0);

                    for (section, units) in sections {
                        ui.label(egui::RichText::new(&section).strong().size(14.0));

                        ui.add_space(3.0);

                        let scroll_id = format!(
                            "dashboard_section_{}_{}",
                            building, section,
                        );

                        egui::ScrollArea::horizontal()
                            .id_salt(scroll_id)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    for unit in units {
                                        let padlock =
                                            padlocks_by_unit.get(&unit.id).copied();

                                        show_unit_tile(ui, unit, padlock);
                                    }
                                });
                            });

                        ui.add_space(10.0);
                    }

                    ui.separator();
                }
            });
    }

    fn refresh(&mut self, context: &mut AppContext) {
        let summary = match context.database().get_report_summary() {
            Ok(summary) => summary,

            Err(error) => {
                self.loaded = true;

                self.set_error(
                    context,
                    format!("Unable to load dashboard summary: {}", error,),
                );

                return;
            }
        };

        let units = match context.database().get_all_units() {
            Ok(units) => units,

            Err(error) => {
                self.loaded = true;

                self.set_error(context, format!("Unable to load unit layout: {}", error,));

                return;
            }
        };

        let padlocks = match context.database().get_all_padlocks() {
            Ok(padlocks) => padlocks,

            Err(error) => {
                self.loaded = true;

                self.set_error(
                    context,
                    format!("Unable to load padlock status: {}", error,),
                );

                return;
            }
        };

        self.summary = Some(summary);
        self.units = units;
        self.padlocks = padlocks;
        self.loaded = true;

        self.data_revision_seen = context.data_revision();

        self.error_message = None;
    }

    fn set_error<S>(&mut self, context: &mut AppContext, message: S)
    where
        S: Into<String>,
    {
        let message = message.into();

        context.set_status_message(message.clone());

        self.error_message = Some(message);
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

fn show_unit_tile(ui: &mut egui::Ui, unit: &Unit, padlock: Option<&Padlock>) {
    let status = if unit.occupied {
        "Occupied"
    } else if unit.needs_cleaned {
        "Needs Cleaned"
    } else {
        "Vacant"
    };

    let fill = if unit.occupied {
        occupied_color()
    } else if unit.needs_cleaned {
        needs_cleaned_color()
    } else {
        vacant_color()
    };

    let has_customer_lock = unit.red_lock || padlock.is_some();

    let mut lock_labels = Vec::new();

    if unit.yellow_lock {
        lock_labels.push("🟨");
    }

    if unit.red_lock {
        lock_labels.push("🟥");
    }

    if padlock.is_some() {
        lock_labels.push("🔒");
    }

    let lock_status = if lock_labels.is_empty() {
        if unit.occupied {
            "⚠ No lock".to_string()
        } else {
            "No lock".to_string()
        }
    } else {
        lock_labels.join("  ")
    };

    let label = format!(
        "{}\n{}\n{}\n{}",
        unit.unit_number,
        status,
        lock_status,
        format_money(unit.monthly_rent_cents,),
    );

    let mut button = egui::Button::new(
        egui::RichText::new(label)
            .strong()
            .color(egui::Color32::WHITE),
    )
    .fill(fill);

    if unit.occupied && !has_customer_lock {
        button = button.stroke(egui::Stroke::new(3.0_f32, warning_color()));
    }

    let tile_width = if matches!(
        UnitSize::from_dimensions(unit.width, unit.length),
        Some(UnitSize::FiveByFive)
    ) {
        92.0_f32
    } else {
        165.0_f32
    };

    let response = ui.add_sized([tile_width, 116.0_f32], button);

    let padlock_details = if let Some(padlock) = padlock {
        format!("Tracked padlock: {}", padlock.serial_number,)
    } else {
        "Tracked padlock: None".to_string()
    };

    let customer_lock_status = if has_customer_lock {
        "Customer lock coverage: Present"
    } else if unit.occupied {
        "Customer lock coverage: MISSING"
    } else {
        "Customer lock coverage: None"
    };

    response.on_hover_text(format!(
        "Unit {}\nBuilding: {}\nSection: {}\nDisplay order: {}\nSize: {}\nStatus: {}\nYellow lock: {}\nRed lock: {}\n{}\n{}\nMonthly rent: {}",
        unit.unit_number,
        if unit.building.is_empty() {
            "Unassigned"
        } else {
            unit.building.as_str()
        },
        if unit.layout_section.trim().is_empty() {
            "Main Section"
        } else {
            unit.layout_section.as_str()
        },
        unit.display_order,
        format_dimensions(unit),
        status,
        if unit.yellow_lock { "Yes" } else { "No" },
        if unit.red_lock { "Yes" } else { "No" },
        padlock_details,
        customer_lock_status,
        format_money(unit.monthly_rent_cents,),
    ));
}

fn vacant_color() -> egui::Color32 {
    egui::Color32::from_rgb(40, 125, 75)
}

fn occupied_color() -> egui::Color32 {
    egui::Color32::from_rgb(155, 55, 55)
}

fn warning_color() -> egui::Color32 {
    egui::Color32::from_rgb(225, 170, 35)
}

fn needs_cleaned_color() -> egui::Color32 {
    egui::Color32::from_rgb(190, 120, 35)
}

fn format_dimensions(unit: &Unit) -> String {
    if let Some(size) = UnitSize::from_dimensions(unit.width, unit.length) {
        return size.label().to_string();
    }

    match (unit.width, unit.length) {
        (Some(width), Some(length)) => {
            format!("{} × {}", format_dimension(width), format_dimension(length),)
        }

        _ => "Unknown".to_string(),
    }
}

fn format_dimension(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

fn format_money(cents: i64) -> String {
    format!("${}.{:02}", cents / 100, cents.abs() % 100,)
}
