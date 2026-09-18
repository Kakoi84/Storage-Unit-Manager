use crate::{
    app::AppContext,
    models::{NewRental, Rental, Tenant, Unit},
};

use eframe::egui;

#[derive(Default)]
pub struct RentalsPage {
    form: RentalForm,
    tenants: Vec<Tenant>,
    vacant_units: Vec<Unit>,
    active_rentals: Vec<Rental>,
    loaded: bool,
    pending_end: Option<PendingEnd>,
    error_message: Option<String>,
}

#[derive(Default)]
struct RentalForm {
    tenant_id: Option<i64>,
    unit_id: Option<i64>,
    start_date: String,
    monthly_rent: String,
    notes: String,
}

#[derive(Debug, Clone)]
struct PendingEnd {
    rental_id: i64,
    tenant_name: String,
    unit_number: String,
    start_date: String,
    end_date: String,
}

impl RentalsPage {
    pub fn show(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        if !self.loaded {
            self.refresh_all(context);
        }

        ui.heading("🔑 Rentals");
        ui.separator();

        self.show_rental_form(ui, context);

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);

        self.show_active_rentals(ui);

        let egui_context = ui.ctx().clone();
        self.show_end_confirmation(&egui_context, context);
    }

    fn show_rental_form(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        ui.heading("Start Rental");

        let selected_tenant_text = self.selected_tenant_text();
        let selected_unit_text = self.selected_unit_text();
        let previous_unit_id = self.form.unit_id;

        egui::Grid::new("start_rental_form")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Tenant:");

                egui::ComboBox::from_id_salt("rental_tenant")
                    .selected_text(selected_tenant_text)
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.form.tenant_id, None, "Select Tenant");

                        for tenant in &self.tenants {
                            ui.selectable_value(
                                &mut self.form.tenant_id,
                                Some(tenant.id),
                                tenant.display_name(),
                            );
                        }
                    });

                ui.end_row();

                ui.label("Storage unit:");

                egui::ComboBox::from_id_salt("rental_unit")
                    .selected_text(selected_unit_text)
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.form.unit_id, None, "Select Unit");

                        for unit in &self.vacant_units {
                            ui.selectable_value(
                                &mut self.form.unit_id,
                                Some(unit.id),
                                unit_display_name(unit),
                            );
                        }
                    });

                ui.end_row();

                ui.label("Start date:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.form.start_date)
                        .hint_text("YYYY-MM-DD")
                        .desired_width(140.0),
                );
                ui.end_row();

                ui.label("Monthly rent:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.form.monthly_rent)
                        .hint_text("0.00")
                        .desired_width(140.0),
                );
                ui.end_row();

                ui.label("Notes:");
                ui.add(
                    egui::TextEdit::multiline(&mut self.form.notes)
                        .desired_width(320.0)
                        .desired_rows(3),
                );
                ui.end_row();
            });

        if self.form.unit_id != previous_unit_id {
            match self.form.unit_id {
                Some(unit_id) => {
                    if let Some(unit) = self.vacant_units.iter().find(|unit| unit.id == unit_id) {
                        self.form.monthly_rent = format_money_input(unit.monthly_rent_cents);
                    }
                }
                None => self.form.monthly_rent.clear(),
            }
        }

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            let can_start = !self.tenants.is_empty() && !self.vacant_units.is_empty();

            if ui
                .add_enabled(can_start, egui::Button::new("Start Rental"))
                .clicked()
            {
                self.start_rental(context);
            }

            if ui.button("Clear").clicked() {
                self.form = RentalForm::default();
                self.error_message = None;
                context.set_status_message("Rental form cleared");
            }

            if ui.button("Refresh").clicked() {
                self.refresh_all(context);

                if self.error_message.is_none() {
                    context.set_status_message("Rental information refreshed");
                }
            }
        });

        if self.tenants.is_empty() {
            ui.add_space(6.0);
            ui.label("No active tenants are available.");
        }

        if self.vacant_units.is_empty() {
            ui.add_space(6.0);
            ui.label("No vacant storage units are available.");
        }

        if let Some(error_message) = &self.error_message {
            ui.add_space(6.0);
            ui.colored_label(egui::Color32::RED, error_message);
        }
    }

    fn show_active_rentals(&mut self, ui: &mut egui::Ui) {
        ui.heading("Active Rentals");

        if self.active_rentals.is_empty() {
            ui.label("No active rentals were found.");
            return;
        }

        let mut requested_end: Option<PendingEnd> = None;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("active_rentals_table")
                    .striped(true)
                    .min_col_width(80.0)
                    .spacing([20.0, 6.0])
                    .show(ui, |ui| {
                        ui.strong("Unit");
                        ui.strong("Tenant");
                        ui.strong("Start Date");
                        ui.strong("Monthly Rent");
                        ui.strong("Actions");
                        ui.end_row();

                        for rental in &self.active_rentals {
                            ui.label(&rental.unit_number);
                            ui.label(&rental.tenant_name);
                            ui.label(&rental.start_date);
                            ui.label(format_money(rental.monthly_rent_cents));

                            if ui.small_button("End Rental").clicked() {
                                requested_end = Some(PendingEnd {
                                    rental_id: rental.id,
                                    tenant_name: rental.tenant_name.clone(),
                                    unit_number: rental.unit_number.clone(),
                                    start_date: rental.start_date.clone(),
                                    end_date: String::new(),
                                });
                            }

                            ui.end_row();
                        }
                    });
            });

        if let Some(pending_end) = requested_end {
            self.pending_end = Some(pending_end);
        }
    }

    fn start_rental(&mut self, context: &mut AppContext) {
        self.error_message = None;

        let Some(tenant_id) = self.form.tenant_id else {
            self.set_error(context, "A tenant must be selected.");
            return;
        };

        let Some(unit_id) = self.form.unit_id else {
            self.set_error(context, "A storage unit must be selected.");
            return;
        };

        let start_date = match validate_iso_date(&self.form.start_date, "Start date") {
            Ok(date) => date,
            Err(error) => {
                self.set_error(context, error);
                return;
            }
        };

        let monthly_rent_cents = match parse_dollars_to_cents(&self.form.monthly_rent) {
            Ok(value) => value,
            Err(error) => {
                self.set_error(context, error);
                return;
            }
        };

        let tenant_name = self
            .tenants
            .iter()
            .find(|tenant| tenant.id == tenant_id)
            .map(Tenant::display_name)
            .unwrap_or_else(|| format!("Tenant ID {tenant_id}"));

        let unit_number = self
            .vacant_units
            .iter()
            .find(|unit| unit.id == unit_id)
            .map(|unit| unit.unit_number.clone())
            .unwrap_or_else(|| format!("Unit ID {unit_id}"));

        let new_rental = NewRental {
            tenant_id,
            unit_id,
            start_date,
            monthly_rent_cents,
            notes: self.form.notes.trim().to_string(),
        };

        match context.database_mut().start_rental(&new_rental) {
            Ok(id) => {
                context.mark_data_changed();
                context.set_status_message(format!(
                    "Started rental for {} in Unit {} — rental ID {}",
                    tenant_name, unit_number, id,
                ));

                self.form = RentalForm::default();
                self.refresh_all(context);
            }
            Err(error) => {
                self.set_error(context, format!("Unable to start rental: {}", error));
            }
        }
    }

    fn show_end_confirmation(&mut self, ctx: &egui::Context, context: &mut AppContext) {
        let Some(mut pending) = self.pending_end.clone() else {
            return;
        };

        let mut confirm_end = false;
        let mut cancel_end = false;

        egui::Window::new("End Rental")
            .id(egui::Id::new("end_rental_confirmation"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!(
                    "End the rental for {} in Unit {}?",
                    pending.tenant_name, pending.unit_number,
                ));

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label("End date:");
                    ui.add(
                        egui::TextEdit::singleline(&mut pending.end_date)
                            .hint_text("YYYY-MM-DD")
                            .desired_width(140.0),
                    );
                });

                ui.add_space(6.0);
                ui.label("The unit will become vacant.");
                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    if ui.button("End Rental").clicked() {
                        confirm_end = true;
                    }

                    if ui.button("Cancel").clicked() {
                        cancel_end = true;
                    }
                });
            });

        if cancel_end {
            self.pending_end = None;
            context.set_status_message("End rental canceled");
            return;
        }

        if confirm_end {
            self.perform_end(pending, context);
            return;
        }

        self.pending_end = Some(pending);
    }

    fn perform_end(&mut self, pending: PendingEnd, context: &mut AppContext) {
        let end_date = match validate_iso_date(&pending.end_date, "End date") {
            Ok(date) => date,
            Err(error) => {
                self.pending_end = Some(pending);
                self.set_error(context, error);
                return;
            }
        };

        if end_date.as_str() < pending.start_date.as_str() {
            self.pending_end = Some(pending);
            self.set_error(context, "End date cannot be before the rental start date.");
            return;
        }

        match context
            .database_mut()
            .end_rental(pending.rental_id, &end_date)
        {
            Ok(true) => {
                self.pending_end = None;
                self.error_message = None;
                context.mark_data_changed();
                context.set_status_message(format!(
                    "Ended rental for {} in Unit {}",
                    pending.tenant_name, pending.unit_number,
                ));
                self.refresh_all(context);
            }
            Ok(false) => {
                self.pending_end = None;
                self.set_error(
                    context,
                    format!(
                        "The rental for Unit {} is no longer active.",
                        pending.unit_number,
                    ),
                );
            }
            Err(error) => {
                self.pending_end = Some(pending);
                self.set_error(context, format!("Unable to end rental: {}", error));
            }
        }
    }

    fn refresh_all(&mut self, context: &mut AppContext) {
        let tenants = match context.database().get_active_tenants() {
            Ok(tenants) => tenants,
            Err(error) => {
                self.loaded = true;
                self.set_error(context, format!("Unable to load active tenants: {}", error));
                return;
            }
        };

        let vacant_units = match context.database().get_vacant_units() {
            Ok(units) => units,
            Err(error) => {
                self.loaded = true;
                self.set_error(context, format!("Unable to load vacant units: {}", error));
                return;
            }
        };

        let active_rentals = match context.database().get_active_rentals() {
            Ok(rentals) => rentals,
            Err(error) => {
                self.loaded = true;
                self.set_error(context, format!("Unable to load active rentals: {}", error));
                return;
            }
        };

        self.tenants = tenants;
        self.vacant_units = vacant_units;
        self.active_rentals = active_rentals;
        self.loaded = true;
        self.error_message = None;
    }

    fn selected_tenant_text(&self) -> String {
        let Some(tenant_id) = self.form.tenant_id else {
            return "Select Tenant".to_string();
        };

        self.tenants
            .iter()
            .find(|tenant| tenant.id == tenant_id)
            .map(Tenant::display_name)
            .unwrap_or_else(|| "Select Tenant".to_string())
    }

    fn selected_unit_text(&self) -> String {
        let Some(unit_id) = self.form.unit_id else {
            return "Select Unit".to_string();
        };

        self.vacant_units
            .iter()
            .find(|unit| unit.id == unit_id)
            .map(unit_display_name)
            .unwrap_or_else(|| "Select Unit".to_string())
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

fn unit_display_name(unit: &Unit) -> String {
    if unit.building.is_empty() {
        format!(
            "{} — {}",
            unit.unit_number,
            format_money(unit.monthly_rent_cents),
        )
    } else {
        format!(
            "{} / {} — {}",
            unit.building,
            unit.unit_number,
            format_money(unit.monthly_rent_cents),
        )
    }
}

fn parse_dollars_to_cents(text: &str) -> Result<i64, String> {
    let cleaned = text.trim().trim_start_matches('$').replace(',', "");

    if cleaned.is_empty() {
        return Err("Monthly rent is required.".to_string());
    }

    if cleaned.starts_with('-') {
        return Err("Monthly rent cannot be negative.".to_string());
    }

    let parts: Vec<&str> = cleaned.split('.').collect();

    if parts.len() > 2 {
        return Err("Monthly rent is not valid.".to_string());
    }

    let dollars = if parts[0].is_empty() {
        0
    } else {
        parts[0]
            .parse::<i64>()
            .map_err(|_| "Monthly rent is not valid.".to_string())?
    };

    let cents = match parts.get(1).copied() {
        None | Some("") => 0,
        Some(value) if value.len() == 1 => {
            value
                .parse::<i64>()
                .map_err(|_| "Monthly rent is not valid.".to_string())?
                * 10
        }
        Some(value) if value.len() == 2 => value
            .parse::<i64>()
            .map_err(|_| "Monthly rent is not valid.".to_string())?,
        Some(_) => {
            return Err("Monthly rent can have no more than two decimal places.".to_string());
        }
    };

    dollars
        .checked_mul(100)
        .and_then(|value| value.checked_add(cents))
        .ok_or_else(|| "Monthly rent is too large.".to_string())
}

fn format_money(cents: i64) -> String {
    format!("${}.{:02}", cents / 100, cents.abs() % 100)
}

fn format_money_input(cents: i64) -> String {
    format!("{}.{:02}", cents / 100, cents.abs() % 100)
}

fn validate_iso_date(text: &str, field_name: &str) -> Result<String, String> {
    let date = text.trim();
    let bytes = date.as_bytes();

    let valid_shape = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes.iter().enumerate().all(|(index, byte)| {
            if index == 4 || index == 7 {
                *byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        });

    if !valid_shape {
        return Err(format!("{} must use YYYY-MM-DD.", field_name));
    }

    let year = date[0..4]
        .parse::<i32>()
        .map_err(|_| format!("{} contains an invalid year.", field_name))?;

    let month = date[5..7]
        .parse::<u32>()
        .map_err(|_| format!("{} contains an invalid month.", field_name))?;

    let day = date[8..10]
        .parse::<u32>()
        .map_err(|_| format!("{} contains an invalid day.", field_name))?;

    if year < 1 {
        return Err(format!("{} contains an invalid year.", field_name));
    }

    let max_day = days_in_month(year, month)
        .ok_or_else(|| format!("{} contains an invalid month.", field_name))?;

    if day == 0 || day > max_day {
        return Err(format!("{} contains an invalid day.", field_name));
    }

    Ok(date.to_string())
}

fn days_in_month(year: i32, month: u32) -> Option<u32> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 if is_leap_year(year) => Some(29),
        2 => Some(28),
        _ => None,
    }
}

fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}
