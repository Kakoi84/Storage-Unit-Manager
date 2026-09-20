use crate::{
    app::AppContext,
    models::{NewUnit, Unit, UnitSize},
};

use eframe::egui;

#[derive(Default)]
pub struct UnitsPage {
    form: UnitForm,
    units: Vec<Unit>,
    archived_units: Vec<Unit>,
    loaded: bool,
    editing_unit_id: Option<i64>,
    pending_archive: Option<PendingArchive>,
    pending_delete: Option<PendingDelete>,
    error_message: Option<String>,
}

#[derive(Debug, Clone)]
struct PendingArchive {
    id: i64,
    unit_number: String,
}

#[derive(Debug, Clone)]
struct PendingDelete {
    id: i64,
    unit_number: String,
}

#[derive(Default)]
struct UnitForm {
    unit_number: String,
    building: String,
    size: UnitSize,
    monthly_rent: String,
    layout_section: String,
    display_order: String,
    yellow_lock: bool,
    red_lock: bool,
    notes: String,
}

impl UnitForm {
    fn from_unit(unit: &Unit) -> Result<Self, String> {
        let size = UnitSize::from_dimensions(unit.width, unit.length).ok_or_else(|| {
            format!(
                "Unit {} has dimensions that do not match a standard size.",
                unit.unit_number,
            )
        })?;

        Ok(Self {
            unit_number: unit.unit_number.clone(),
            building: unit.building.clone(),
            size,
            monthly_rent: format_money_input(unit.monthly_rent_cents),
            layout_section: unit.layout_section.clone(),
            display_order: unit.display_order.to_string(),
            yellow_lock: unit.yellow_lock,
            red_lock: unit.red_lock,
            notes: unit.notes.clone(),
        })
    }

    fn to_new_unit(&self) -> Result<NewUnit, String> {
        let unit_number = self.unit_number.trim().to_string();

        if unit_number.is_empty() {
            return Err("Unit number is required.".to_string());
        }

        let monthly_rent_cents = parse_dollars_to_cents(&self.monthly_rent)?;

        let display_order = parse_display_order(&self.display_order)?;

        let (width, length) = self.size.dimensions();

        Ok(NewUnit {
            unit_number,
            building: self.building.trim().to_string(),
            width: Some(width),
            length: Some(length),
            monthly_rent_cents,
            layout_section: self.layout_section.trim().to_string(),
            display_order,
            yellow_lock: self.yellow_lock,
            red_lock: self.red_lock,
            notes: self.notes.trim().to_string(),
        })
    }
}

impl UnitsPage {
    pub fn show(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        if !self.loaded {
            self.refresh_units(context);
        }

        ui.heading("📦 Units");
        ui.separator();

        self.show_unit_form(ui, context);

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);

        self.show_unit_list(ui, context);

        ui.add_space(14.0);

        self.show_archived_units(ui, context);

        let egui_context = ui.ctx().clone();

        self.show_archive_confirmation(&egui_context, context);

        self.show_delete_confirmation(&egui_context, context);
    }

    fn show_unit_form(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        let editing = self.editing_unit_id.is_some();

        if editing {
            ui.heading("Edit Storage Unit");
        } else {
            ui.heading("Add Storage Unit");
        }

        egui::Grid::new("unit_form")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Unit number:");

                ui.text_edit_singleline(&mut self.form.unit_number);

                ui.end_row();

                ui.label("Building:");

                ui.text_edit_singleline(&mut self.form.building);

                ui.end_row();

                ui.label("Unit size:");

                egui::ComboBox::from_id_salt("unit_size")
                    .selected_text(self.form.size.label())
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        for size in UnitSize::ALL {
                            ui.selectable_value(&mut self.form.size, size, size.label());
                        }
                    });

                ui.end_row();

                ui.label("Monthly rent:");

                ui.text_edit_singleline(&mut self.form.monthly_rent);

                ui.end_row();

                ui.label("Layout section:");

                ui.add(
                    egui::TextEdit::singleline(&mut self.form.layout_section)
                        .desired_width(220.0)
                        .hint_text("Example: North Side or East End"),
                );

                ui.end_row();

                ui.label("Display order:");

                ui.add(
                    egui::TextEdit::singleline(&mut self.form.display_order)
                        .desired_width(80.0)
                        .hint_text("0"),
                );

                ui.end_row();

                ui.label("Unit locks:");

                ui.horizontal(|ui| {
                    ui.colored_label(yellow_lock_color(), "■");
                    ui.checkbox(&mut self.form.yellow_lock, "Yellow Lock");

                    ui.add_space(12.0_f32);

                    ui.colored_label(red_lock_color(), "■");
                    ui.checkbox(&mut self.form.red_lock, "Red Lock");
                });

                ui.end_row();

                ui.label("Notes:");

                ui.add(
                    egui::TextEdit::multiline(&mut self.form.notes)
                        .desired_width(300.0)
                        .desired_rows(3),
                );

                ui.end_row();
            });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if editing {
                if ui.button("Save Changes").clicked() {
                    self.save_changes(context);
                }

                if ui.button("Cancel").clicked() {
                    self.cancel_edit(context);
                }
            } else {
                if ui.button("Add Unit").clicked() {
                    self.add_unit(context);
                }

                if ui.button("Clear").clicked() {
                    self.form = UnitForm::default();

                    self.error_message = None;

                    context.set_status_message("Unit form cleared");
                }
            }
        });

        if let Some(error_message) = &self.error_message {
            ui.add_space(6.0);

            ui.colored_label(egui::Color32::RED, error_message);
        }
    }

    fn show_unit_list(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        ui.horizontal(|ui| {
            ui.heading("Active Units");

            if ui.button("Refresh").clicked() {
                self.refresh_units(context);

                if self.error_message.is_none() {
                    context.set_status_message("Unit list refreshed");
                }
            }
        });

        if self.units.is_empty() {
            ui.label("No active storage units were found.");

            return;
        }

        let mut requested_edit_id: Option<i64> = None;

        let mut requested_archive: Option<PendingArchive> = None;

        let mut requested_delete: Option<PendingDelete> = None;

        let mut requested_cleaning_status: Option<(i64, bool, String)> = None;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("units_table")
                    .striped(true)
                    .min_col_width(70.0)
                    .spacing([20.0, 6.0])
                    .show(ui, |ui| {
                        ui.strong("Unit");
                        ui.strong("Building");
                        ui.strong("Size");
                        ui.strong("Rent");
                        ui.strong("Layout");
                        ui.strong("Status");
                        ui.strong("Actions");
                        ui.end_row();

                        for unit in &self.units {
                            ui.label(&unit.unit_number);

                            if unit.building.is_empty() {
                                ui.label("—");
                            } else {
                                ui.label(&unit.building);
                            }

                            ui.label(format_dimensions(unit));

                            ui.label(format_money(unit.monthly_rent_cents));

                            ui.label(format_layout(unit));

                            if unit.occupied {
                                ui.label("Occupied");
                            } else if unit.needs_cleaned {
                                ui.colored_label(
                                    egui::Color32::from_rgb(190, 120, 35),
                                    "Needs Cleaned",
                                );
                            } else {
                                ui.label("Vacant");
                            }

                            ui.horizontal(|ui| {
                                if ui.small_button("✏ Edit").clicked() {
                                    requested_edit_id = Some(unit.id);
                                }

                                if !unit.occupied {
                                    if unit.needs_cleaned {
                                        if ui.small_button("✓ Mark Cleaned").clicked() {
                                            requested_cleaning_status =
                                                Some((unit.id, false, unit.unit_number.clone()));
                                        }
                                    } else if ui.small_button("🧹 Needs Cleaned").clicked() {
                                        requested_cleaning_status =
                                            Some((unit.id, true, unit.unit_number.clone()));
                                    }
                                }

                                if ui.small_button("📦 Archive").clicked() {
                                    requested_archive = Some(PendingArchive {
                                        id: unit.id,
                                        unit_number: unit.unit_number.clone(),
                                    });
                                }

                                if ui.small_button("🗑 Delete").clicked() {
                                    requested_delete = Some(PendingDelete {
                                        id: unit.id,
                                        unit_number: unit.unit_number.clone(),
                                    });
                                }
                            });

                            ui.end_row();
                        }
                    });
            });

        if let Some(id) = requested_edit_id {
            self.begin_edit(id, context);
        }

        if let Some(pending_archive) = requested_archive {
            self.pending_archive = Some(pending_archive);
        }

        if let Some(pending_delete) = requested_delete {
            self.pending_delete = Some(pending_delete);
        }

        if let Some((id, needs_cleaned, unit_number)) = requested_cleaning_status {
            self.set_cleaning_status(id, needs_cleaned, &unit_number, context);
        }
    }

    fn set_cleaning_status(
        &mut self,
        id: i64,
        needs_cleaned: bool,
        unit_number: &str,
        context: &mut AppContext,
    ) {
        match context.database().set_unit_needs_cleaned(id, needs_cleaned) {
            Ok(true) => {
                self.error_message = None;

                context.mark_data_changed();

                if needs_cleaned {
                    context
                        .set_status_message(format!("Unit {} marked Needs Cleaned", unit_number,));
                } else {
                    context.set_status_message(format!(
                        "Unit {} marked Cleaned and Vacant",
                        unit_number,
                    ));
                }

                self.refresh_units(context);
            }

            Ok(false) => {
                self.set_error(
                    context,
                    format!(
                        "Unit {} could not be updated. It may be occupied or no longer exist.",
                        unit_number,
                    ),
                );
            }

            Err(error) => {
                self.set_error(
                    context,
                    format!(
                        "Unable to update cleaning status for Unit {}: {}",
                        unit_number, error,
                    ),
                );
            }
        }
    }

    fn show_archived_units(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        let title = format!("Archived Units ({})", self.archived_units.len(),);

        egui::CollapsingHeader::new(title)
            .default_open(false)
            .show(ui, |ui| {
                if self.archived_units.is_empty() {
                    ui.label("No archived units were found.");

                    return;
                }

                let mut requested_restore: Option<(i64, String)> = None;

                egui::Grid::new("archived_units_table")
                    .striped(true)
                    .min_col_width(70.0)
                    .spacing([20.0, 6.0])
                    .show(ui, |ui| {
                        ui.strong("Unit");
                        ui.strong("Building");
                        ui.strong("Size");
                        ui.strong("Rent");
                        ui.strong("Layout");
                        ui.strong("Action");
                        ui.end_row();

                        for unit in &self.archived_units {
                            ui.label(&unit.unit_number);

                            if unit.building.is_empty() {
                                ui.label("—");
                            } else {
                                ui.label(&unit.building);
                            }

                            ui.label(format_dimensions(unit));

                            ui.label(format_money(unit.monthly_rent_cents));

                            ui.label(format_layout(unit));

                            if ui.small_button("↩ Restore").clicked() {
                                requested_restore = Some((unit.id, unit.unit_number.clone()));
                            }

                            ui.end_row();
                        }
                    });

                if let Some((id, unit_number)) = requested_restore {
                    self.restore_unit(id, &unit_number, context);
                }
            });
    }

    fn begin_edit(&mut self, id: i64, context: &mut AppContext) {
        let unit = self.units.iter().find(|unit| unit.id == id).cloned();

        let Some(unit) = unit else {
            self.set_error(context, format!("Unable to find Unit ID {}.", id,));

            return;
        };

        match UnitForm::from_unit(&unit) {
            Ok(form) => {
                self.form = form;
                self.editing_unit_id = Some(id);
                self.error_message = None;

                context.set_status_message(format!("Editing Unit {}", unit.unit_number,));
            }

            Err(error) => {
                self.set_error(context, error);
            }
        }
    }

    fn cancel_edit(&mut self, context: &mut AppContext) {
        self.editing_unit_id = None;
        self.form = UnitForm::default();
        self.error_message = None;

        context.set_status_message("Unit edit canceled");
    }

    fn add_unit(&mut self, context: &mut AppContext) {
        self.error_message = None;

        let new_unit = match self.form.to_new_unit() {
            Ok(unit) => unit,

            Err(error) => {
                self.set_error(context, error);

                return;
            }
        };

        match context.database().add_unit(&new_unit) {
            Ok(id) => {
                context.mark_data_changed();

                context.set_status_message(format!(
                    "Added Unit {} — database ID {}",
                    new_unit.unit_number, id,
                ));

                self.form = UnitForm::default();

                self.refresh_units(context);
            }

            Err(error) => {
                self.set_error(
                    context,
                    format!("Unable to add Unit {}: {}", new_unit.unit_number, error,),
                );
            }
        }
    }

    fn save_changes(&mut self, context: &mut AppContext) {
        self.error_message = None;

        let Some(id) = self.editing_unit_id else {
            self.set_error(context, "No storage unit is selected for editing.");

            return;
        };

        let updated_unit = match self.form.to_new_unit() {
            Ok(unit) => unit,

            Err(error) => {
                self.set_error(context, error);

                return;
            }
        };

        match context.database().update_unit(id, &updated_unit) {
            Ok(true) => {
                context.mark_data_changed();

                context.set_status_message(format!("Updated Unit {}", updated_unit.unit_number,));

                self.editing_unit_id = None;

                self.form = UnitForm::default();

                self.refresh_units(context);
            }

            Ok(false) => {
                self.set_error(context, format!("Unit ID {} no longer exists.", id,));
            }

            Err(error) => {
                self.set_error(
                    context,
                    format!(
                        "Unable to update Unit {}: {}",
                        updated_unit.unit_number, error,
                    ),
                );
            }
        }
    }

    fn show_archive_confirmation(&mut self, ctx: &egui::Context, context: &mut AppContext) {
        let Some(pending) = self.pending_archive.clone() else {
            return;
        };

        let mut confirm_archive = false;
        let mut cancel_archive = false;

        egui::Window::new("Archive Storage Unit")
            .id(egui::Id::new("archive_unit_confirmation"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!("Archive Unit {}?", pending.unit_number,));

                ui.add_space(6.0);

                ui.label("The unit will be hidden from active lists and the dashboard.");

                ui.label("Rental and payment history will be preserved.");

                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    if ui.button("Archive Unit").clicked() {
                        confirm_archive = true;
                    }

                    if ui.button("Cancel").clicked() {
                        cancel_archive = true;
                    }
                });
            });

        if cancel_archive {
            self.pending_archive = None;

            context.set_status_message("Unit archive canceled");
        }

        if confirm_archive {
            self.perform_archive(pending, context);
        }
    }

    fn perform_archive(&mut self, pending: PendingArchive, context: &mut AppContext) {
        match context.database().archive_unit(pending.id) {
            Ok(true) => {
                if self.editing_unit_id == Some(pending.id) {
                    self.editing_unit_id = None;

                    self.form = UnitForm::default();
                }

                self.pending_archive = None;
                self.error_message = None;

                context.mark_data_changed();

                context.set_status_message(format!("Archived Unit {}", pending.unit_number,));

                self.refresh_units(context);
            }

            Ok(false) => {
                self.pending_archive = None;

                self.set_error(
                    context,
                    format!(
                        "Unit {} is already archived or no longer exists.",
                        pending.unit_number,
                    ),
                );
            }

            Err(error) => {
                self.pending_archive = None;

                self.set_error(
                    context,
                    format!("Unable to archive Unit {}: {}", pending.unit_number, error,),
                );
            }
        }
    }

    fn restore_unit(&mut self, id: i64, unit_number: &str, context: &mut AppContext) {
        match context.database().restore_unit(id) {
            Ok(true) => {
                self.error_message = None;

                context.mark_data_changed();

                context.set_status_message(format!("Restored Unit {}", unit_number,));

                self.refresh_units(context);
            }

            Ok(false) => {
                self.set_error(
                    context,
                    format!("Unit {} is not archived or no longer exists.", unit_number,),
                );
            }

            Err(error) => {
                self.set_error(
                    context,
                    format!("Unable to restore Unit {}: {}", unit_number, error,),
                );
            }
        }
    }

    fn show_delete_confirmation(&mut self, ctx: &egui::Context, context: &mut AppContext) {
        let Some(pending) = self.pending_delete.clone() else {
            return;
        };

        let mut confirm_delete = false;
        let mut cancel_delete = false;

        egui::Window::new("Delete Storage Unit")
            .id(egui::Id::new("delete_unit_confirmation"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!("Permanently delete Unit {}?", pending.unit_number,));

                ui.add_space(6.0);

                ui.label("Only units without rental history can be deleted.");

                ui.label("This action cannot be undone.");

                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    if ui.button("🔴 Delete Unit").clicked() {
                        confirm_delete = true;
                    }

                    if ui.button("Cancel").clicked() {
                        cancel_delete = true;
                    }
                });
            });

        if cancel_delete {
            self.pending_delete = None;

            context.set_status_message("Unit deletion canceled");
        }

        if confirm_delete {
            self.perform_delete(pending, context);
        }
    }

    fn perform_delete(&mut self, pending: PendingDelete, context: &mut AppContext) {
        match context.database().delete_unit(pending.id) {
            Ok(true) => {
                if self.editing_unit_id == Some(pending.id) {
                    self.editing_unit_id = None;

                    self.form = UnitForm::default();
                }

                self.pending_delete = None;
                self.error_message = None;

                context.mark_data_changed();

                context.set_status_message(format!("Deleted Unit {}", pending.unit_number,));

                self.refresh_units(context);
            }

            Ok(false) => {
                self.pending_delete = None;

                self.set_error(
                    context,
                    format!("Unit {} no longer exists.", pending.unit_number,),
                );
            }

            Err(error) => {
                self.pending_delete = None;

                self.set_error(
                    context,
                    format!("Unable to delete Unit {}: {}", pending.unit_number, error,),
                );
            }
        }
    }

    fn refresh_units(&mut self, context: &mut AppContext) {
        let units = match context.database().get_all_units() {
            Ok(units) => units,

            Err(error) => {
                self.loaded = true;

                self.set_error(context, format!("Unable to load active units: {}", error,));

                return;
            }
        };

        let archived_units = match context.database().get_archived_units() {
            Ok(units) => units,

            Err(error) => {
                self.loaded = true;

                self.set_error(
                    context,
                    format!("Unable to load archived units: {}", error,),
                );

                return;
            }
        };

        self.units = units;
        self.archived_units = archived_units;
        self.loaded = true;
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

fn parse_display_order(text: &str) -> Result<i64, String> {
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return Ok(0);
    }

    let value = trimmed
        .parse::<i64>()
        .map_err(|_| "Display order must be a whole number.".to_string())?;

    if value < 0 {
        return Err("Display order cannot be negative.".to_string());
    }

    Ok(value)
}

fn format_layout(unit: &Unit) -> String {
    let section = if unit.layout_section.trim().is_empty() {
        "Main Section"
    } else {
        unit.layout_section.trim()
    };

    if unit.display_order == 0 {
        section.to_string()
    } else {
        format!("{} #{}", section, unit.display_order)
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
    format!("${}.{:02}", cents / 100, cents.abs() % 100,)
}

fn format_money_input(cents: i64) -> String {
    format!("{}.{:02}", cents / 100, cents.abs() % 100,)
}

fn yellow_lock_color() -> egui::Color32 {
    egui::Color32::from_rgb(235, 205, 45)
}

fn red_lock_color() -> egui::Color32 {
    egui::Color32::from_rgb(205, 55, 55)
}

fn format_dimensions(unit: &Unit) -> String {
    if let Some(size) = UnitSize::from_dimensions(unit.width, unit.length) {
        return size.label().to_string();
    }

    match (unit.width, unit.length) {
        (Some(width), Some(length)) => {
            format!("{} × {}", format_dimension(width), format_dimension(length),)
        }

        (Some(width), None) => {
            format!("Width: {}", format_dimension(width),)
        }

        (None, Some(length)) => {
            format!("Length: {}", format_dimension(length),)
        }

        (None, None) => "—".to_string(),
    }
}

fn format_dimension(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}
