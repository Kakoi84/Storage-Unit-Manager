use crate::{
    app::AppContext,
    models::{NewPayment, Payment, PaymentMethod, Rental},
};

use eframe::egui;

#[derive(Default)]
pub struct PaymentsPage {
    form: PaymentForm,
    active_rentals: Vec<Rental>,
    payments: Vec<Payment>,
    loaded: bool,
    error_message: Option<String>,
}

#[derive(Default)]
struct PaymentForm {
    rental_id: Option<i64>,
    payment_date: String,
    amount: String,
    payment_method: PaymentMethod,
    reference: String,
    notes: String,
}

impl PaymentsPage {
    pub fn show(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        if !self.loaded {
            self.refresh_all(context);
        }

        ui.heading("💲 Payments");
        ui.separator();

        self.show_payment_form(ui, context);

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);

        self.show_payment_history(ui, context);
    }

    fn show_payment_form(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        ui.heading("Record Payment");

        let selected_rental_text = self.selected_rental_text();
        let previous_rental_id = self.form.rental_id;

        egui::Grid::new("payment_form")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Rental:");

                egui::ComboBox::from_id_salt("payment_rental")
                    .selected_text(selected_rental_text)
                    .width(280.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.form.rental_id, None, "Select Rental");

                        for rental in &self.active_rentals {
                            ui.selectable_value(
                                &mut self.form.rental_id,
                                Some(rental.id),
                                rental_display_name(rental),
                            );
                        }
                    });

                ui.end_row();

                ui.label("Payment date:");

                ui.add(
                    egui::TextEdit::singleline(&mut self.form.payment_date)
                        .hint_text("YYYY-MM-DD")
                        .desired_width(140.0),
                );

                ui.end_row();

                ui.label("Amount:");

                ui.add(
                    egui::TextEdit::singleline(&mut self.form.amount)
                        .hint_text("0.00")
                        .desired_width(140.0),
                );

                ui.end_row();

                ui.label("Method:");

                egui::ComboBox::from_id_salt("payment_method")
                    .selected_text(self.form.payment_method.label())
                    .width(160.0)
                    .show_ui(ui, |ui| {
                        for method in PaymentMethod::ALL {
                            ui.selectable_value(
                                &mut self.form.payment_method,
                                method,
                                method.label(),
                            );
                        }
                    });

                ui.end_row();

                ui.label("Reference:");

                ui.add(
                    egui::TextEdit::singleline(&mut self.form.reference)
                        .hint_text("Check number or transaction ID")
                        .desired_width(280.0),
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

        if self.form.rental_id != previous_rental_id {
            match self.form.rental_id {
                Some(rental_id) => {
                    if let Some(rental) = self
                        .active_rentals
                        .iter()
                        .find(|rental| rental.id == rental_id)
                    {
                        self.form.amount = format_money_input(rental.monthly_rent_cents);
                    }
                }
                None => self.form.amount.clear(),
            }
        }

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            let can_record = !self.active_rentals.is_empty();

            if ui
                .add_enabled(can_record, egui::Button::new("Record Payment"))
                .clicked()
            {
                self.record_payment(context);
            }

            if ui.button("Clear").clicked() {
                self.form = PaymentForm::default();
                self.error_message = None;
                context.set_status_message("Payment form cleared");
            }

            if ui.button("Refresh").clicked() {
                self.refresh_all(context);

                if self.error_message.is_none() {
                    context.set_status_message("Payment information refreshed");
                }
            }
        });

        if self.active_rentals.is_empty() {
            ui.add_space(6.0);
            ui.label("No active rentals are available.");
        }

        if let Some(error_message) = &self.error_message {
            ui.add_space(6.0);
            ui.colored_label(egui::Color32::RED, error_message);
        }
    }

    fn show_payment_history(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        ui.horizontal(|ui| {
            ui.heading("Payment History");

            if ui.button("Refresh").clicked() {
                self.refresh_all(context);

                if self.error_message.is_none() {
                    context.set_status_message("Payment history refreshed");
                }
            }
        });

        if self.payments.is_empty() {
            ui.label("No payments have been recorded.");
            return;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("payments_table")
                    .striped(true)
                    .min_col_width(80.0)
                    .spacing([20.0, 6.0])
                    .show(ui, |ui| {
                        ui.strong("Date");
                        ui.strong("Tenant");
                        ui.strong("Unit");
                        ui.strong("Amount");
                        ui.strong("Method");
                        ui.strong("Reference");
                        ui.end_row();

                        for payment in &self.payments {
                            ui.label(&payment.payment_date);
                            ui.label(&payment.tenant_name);
                            ui.label(&payment.unit_number);
                            ui.label(format_money(payment.amount_cents));
                            ui.label(payment.payment_method.label());

                            if payment.reference.is_empty() {
                                ui.label("—");
                            } else {
                                ui.label(&payment.reference);
                            }

                            ui.end_row();
                        }
                    });
            });
    }

    fn record_payment(&mut self, context: &mut AppContext) {
        self.error_message = None;

        let Some(rental_id) = self.form.rental_id else {
            self.set_error(context, "A rental must be selected.");
            return;
        };

        let payment_date = match validate_iso_date(&self.form.payment_date, "Payment date") {
            Ok(date) => date,
            Err(error) => {
                self.set_error(context, error);
                return;
            }
        };

        let amount_cents = match parse_dollars_to_cents(&self.form.amount) {
            Ok(value) => value,
            Err(error) => {
                self.set_error(context, error);
                return;
            }
        };

        let rental_description = self
            .active_rentals
            .iter()
            .find(|rental| rental.id == rental_id)
            .map(rental_display_name)
            .unwrap_or_else(|| format!("Rental ID {rental_id}"));

        let new_payment = NewPayment {
            rental_id,
            payment_date,
            amount_cents,
            payment_method: self.form.payment_method,
            reference: self.form.reference.trim().to_string(),
            notes: self.form.notes.trim().to_string(),
        };

        match context.database().record_payment(&new_payment) {
            Ok(id) => {
                context.mark_data_changed();
                context.set_status_message(format!(
                    "Recorded {} payment for {} — payment ID {}",
                    format_money(new_payment.amount_cents),
                    rental_description,
                    id,
                ));

                self.form = PaymentForm::default();
                self.refresh_all(context);
            }
            Err(error) => {
                self.set_error(context, format!("Unable to record payment: {}", error));
            }
        }
    }

    fn refresh_all(&mut self, context: &mut AppContext) {
        let active_rentals = match context.database().get_active_rentals() {
            Ok(rentals) => rentals,
            Err(error) => {
                self.loaded = true;
                self.set_error(context, format!("Unable to load active rentals: {}", error));
                return;
            }
        };

        let payments = match context.database().get_all_payments() {
            Ok(payments) => payments,
            Err(error) => {
                self.loaded = true;
                self.set_error(context, format!("Unable to load payments: {}", error));
                return;
            }
        };

        self.active_rentals = active_rentals;
        self.payments = payments;
        self.loaded = true;
        self.error_message = None;
    }

    fn selected_rental_text(&self) -> String {
        let Some(rental_id) = self.form.rental_id else {
            return "Select Rental".to_string();
        };

        self.active_rentals
            .iter()
            .find(|rental| rental.id == rental_id)
            .map(rental_display_name)
            .unwrap_or_else(|| "Select Rental".to_string())
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

fn rental_display_name(rental: &Rental) -> String {
    format!(
        "{} — Unit {} — {}",
        rental.tenant_name,
        rental.unit_number,
        format_money(rental.monthly_rent_cents),
    )
}

fn parse_dollars_to_cents(text: &str) -> Result<i64, String> {
    let cleaned = text.trim().trim_start_matches('$').replace(',', "");

    if cleaned.is_empty() {
        return Err("Payment amount is required.".to_string());
    }

    if cleaned.starts_with('-') {
        return Err("Payment amount cannot be negative.".to_string());
    }

    let parts: Vec<&str> = cleaned.split('.').collect();

    if parts.len() > 2 {
        return Err("Payment amount is not valid.".to_string());
    }

    let dollars = if parts[0].is_empty() {
        0
    } else {
        parts[0]
            .parse::<i64>()
            .map_err(|_| "Payment amount is not valid.".to_string())?
    };

    let cents = match parts.get(1).copied() {
        None | Some("") => 0,
        Some(value) if value.len() == 1 => {
            value
                .parse::<i64>()
                .map_err(|_| "Payment amount is not valid.".to_string())?
                * 10
        }
        Some(value) if value.len() == 2 => value
            .parse::<i64>()
            .map_err(|_| "Payment amount is not valid.".to_string())?,
        Some(_) => {
            return Err("Payment amount can have no more than two decimal places.".to_string());
        }
    };

    let amount = dollars
        .checked_mul(100)
        .and_then(|value| value.checked_add(cents))
        .ok_or_else(|| "Payment amount is too large.".to_string())?;

    if amount <= 0 {
        return Err("Payment amount must be greater than zero.".to_string());
    }

    Ok(amount)
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
