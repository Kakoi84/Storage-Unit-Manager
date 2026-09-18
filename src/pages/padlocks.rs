use crate::{
    app::AppContext,
    models::{NewPadlock, Padlock, Unit},
};

use eframe::egui;

#[derive(Default)]
pub struct PadlocksPage {
    form: PadlockForm,
    padlocks: Vec<Padlock>,
    units: Vec<Unit>,
    loaded: bool,
    data_revision_seen: u64,
    editing_padlock_id: Option<i64>,
    pending_delete: Option<PendingDelete>,
    error_message: Option<String>,
}

#[derive(Debug, Clone)]
struct PendingDelete {
    id: i64,
    serial_number: String,
}

#[derive(Default)]
struct PadlockForm {
    serial_number: String,
    combination: String,
    selected_unit_id: Option<i64>,
    notes: String,
}

impl PadlockForm {
    fn from_padlock(padlock: &Padlock) -> Self {
        Self {
            serial_number: padlock.serial_number.clone(),
            combination: padlock.combination.clone(),
            selected_unit_id: padlock.unit_id,
            notes: padlock.notes.clone(),
        }
    }

    fn to_new_padlock(&self) -> Result<NewPadlock, String> {
        let serial_number = self.serial_number.trim().to_string();

        if serial_number.is_empty() {
            return Err("Padlock serial number is required.".to_string());
        }

        let combination = self.combination.trim().to_string();

        if combination.is_empty() {
            return Err("Padlock combination is required.".to_string());
        }

        Ok(NewPadlock {
            serial_number,
            combination,
            unit_id: self.selected_unit_id,
            notes: self.notes.trim().to_string(),
        })
    }
}

impl PadlocksPage {
    pub fn show(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        if !self.loaded || self.data_revision_seen != context.data_revision() {
            self.refresh(context);
        }

        ui.heading("🔒 Padlocks");
        ui.separator();

        self.show_form(ui, context);

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);

        self.show_padlock_list(ui, context);

        let egui_context = ui.ctx().clone();

        self.show_delete_confirmation(&egui_context, context);
    }

    fn show_form(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        if self.editing_padlock_id.is_some() {
            ui.heading("Edit Padlock");
        } else {
            ui.heading("Add Padlock");
        }

        egui::Grid::new("padlock_form")
            .num_columns(2)
            .spacing([18.0, 8.0])
            .show(ui, |ui| {
                ui.label("Serial number:");

                ui.add(
                    egui::TextEdit::singleline(&mut self.form.serial_number).desired_width(260.0),
                );

                ui.end_row();

                ui.label("Combination:");

                ui.add(egui::TextEdit::singleline(&mut self.form.combination).desired_width(180.0));

                ui.end_row();

                ui.label("Current unit:");

                let selected_text = self.selected_unit_text();

                egui::ComboBox::from_id_salt("padlock_unit_assignment")
                    .selected_text(selected_text)
                    .width(260.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.form.selected_unit_id,
                            None,
                            "Inventory / Not Assigned",
                        );

                        for unit in &self.units {
                            let label = if unit.building.is_empty() {
                                format!("Unit {}", unit.unit_number,)
                            } else {
                                format!("Unit {} — {}", unit.unit_number, unit.building,)
                            };

                            ui.selectable_value(
                                &mut self.form.selected_unit_id,
                                Some(unit.id),
                                label,
                            );
                        }
                    });

                ui.end_row();

                ui.label("Notes:");

                ui.add(
                    egui::TextEdit::multiline(&mut self.form.notes)
                        .desired_width(320.0)
                        .desired_rows(3),
                );

                ui.end_row();
            });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if self.editing_padlock_id.is_some() {
                if ui.button("Save Changes").clicked() {
                    self.save_changes(context);
                }

                if ui.button("Cancel").clicked() {
                    self.cancel_edit(context);
                }
            } else {
                if ui.button("Add Padlock").clicked() {
                    self.add_padlock(context);
                }

                if ui.button("Clear").clicked() {
                    self.form = PadlockForm::default();

                    self.error_message = None;

                    context.set_status_message("Padlock form cleared");
                }
            }
        });

        if let Some(error_message) = &self.error_message {
            ui.add_space(6.0);

            ui.colored_label(egui::Color32::RED, error_message);
        }
    }

    fn show_padlock_list(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        ui.horizontal(|ui| {
            ui.heading("Padlock Inventory");

            if ui.button("Refresh").clicked() {
                self.refresh(context);

                if self.error_message.is_none() {
                    context.set_status_message("Padlock list refreshed");
                }
            }
        });

        if self.padlocks.is_empty() {
            ui.label("No padlocks have been added.");

            return;
        }

        let mut requested_edit_id: Option<i64> = None;

        let mut requested_delete: Option<PendingDelete> = None;

        egui::ScrollArea::vertical()
            .id_salt("padlock_list_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("padlocks_table")
                    .striped(true)
                    .min_col_width(85.0)
                    .spacing([20.0, 7.0])
                    .show(ui, |ui| {
                        ui.strong("Serial Number");
                        ui.strong("Combination");
                        ui.strong("Current Location");
                        ui.strong("Status");
                        ui.strong("Notes");
                        ui.strong("Actions");
                        ui.end_row();

                        for padlock in &self.padlocks {
                            ui.monospace(&padlock.serial_number);

                            ui.monospace(&padlock.combination);

                            if let Some(unit_number) = &padlock.unit_number {
                                ui.label(format!("Unit {}", unit_number,));

                                ui.colored_label(egui::Color32::from_rgb(155, 95, 35), "In Use");
                            } else {
                                ui.label("Inventory");

                                ui.colored_label(egui::Color32::from_rgb(40, 125, 75), "Available");
                            }

                            if padlock.notes.is_empty() {
                                ui.label("—");
                            } else {
                                ui.label(&padlock.notes);
                            }

                            ui.horizontal(|ui| {
                                if ui.small_button("✏ Edit").clicked() {
                                    requested_edit_id = Some(padlock.id);
                                }

                                if ui.small_button("🗑 Delete").clicked() {
                                    requested_delete = Some(PendingDelete {
                                        id: padlock.id,
                                        serial_number: padlock.serial_number.clone(),
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

        if let Some(pending) = requested_delete {
            self.pending_delete = Some(pending);
        }
    }

    fn selected_unit_text(&self) -> String {
        let Some(unit_id) = self.form.selected_unit_id else {
            return "Inventory / Not Assigned".to_string();
        };

        self.units
            .iter()
            .find(|unit| unit.id == unit_id)
            .map(|unit| {
                if unit.building.is_empty() {
                    format!("Unit {}", unit.unit_number,)
                } else {
                    format!("Unit {} — {}", unit.unit_number, unit.building,)
                }
            })
            .unwrap_or_else(|| "Unknown Unit".to_string())
    }

    fn begin_edit(&mut self, id: i64, context: &mut AppContext) {
        let padlock = self
            .padlocks
            .iter()
            .find(|padlock| padlock.id == id)
            .cloned();

        let Some(padlock) = padlock else {
            self.set_error(context, format!("Unable to find Padlock ID {}.", id,));

            return;
        };

        self.form = PadlockForm::from_padlock(&padlock);

        self.editing_padlock_id = Some(id);

        self.error_message = None;

        context.set_status_message(format!("Editing Padlock {}", padlock.serial_number,));
    }

    fn cancel_edit(&mut self, context: &mut AppContext) {
        self.editing_padlock_id = None;
        self.form = PadlockForm::default();
        self.error_message = None;

        context.set_status_message("Padlock edit canceled");
    }

    fn add_padlock(&mut self, context: &mut AppContext) {
        self.error_message = None;

        let new_padlock = match self.form.to_new_padlock() {
            Ok(padlock) => padlock,

            Err(error) => {
                self.set_error(context, error);

                return;
            }
        };

        match context.database().add_padlock(&new_padlock) {
            Ok(id) => {
                context.mark_data_changed();

                context.set_status_message(format!(
                    "Added Padlock {} — database ID {}",
                    new_padlock.serial_number, id,
                ));

                self.form = PadlockForm::default();

                self.refresh(context);
            }

            Err(error) => {
                self.set_error(
                    context,
                    format!(
                        "Unable to add Padlock {}: {}",
                        new_padlock.serial_number, error,
                    ),
                );
            }
        }
    }

    fn save_changes(&mut self, context: &mut AppContext) {
        self.error_message = None;

        let Some(id) = self.editing_padlock_id else {
            self.set_error(context, "No padlock is selected for editing.");

            return;
        };

        let updated_padlock = match self.form.to_new_padlock() {
            Ok(padlock) => padlock,

            Err(error) => {
                self.set_error(context, error);

                return;
            }
        };

        match context.database().update_padlock(id, &updated_padlock) {
            Ok(true) => {
                context.mark_data_changed();

                context.set_status_message(format!(
                    "Updated Padlock {}",
                    updated_padlock.serial_number,
                ));

                self.editing_padlock_id = None;

                self.form = PadlockForm::default();

                self.refresh(context);
            }

            Ok(false) => {
                self.set_error(context, format!("Padlock ID {} no longer exists.", id,));
            }

            Err(error) => {
                self.set_error(
                    context,
                    format!(
                        "Unable to update Padlock {}: {}",
                        updated_padlock.serial_number, error,
                    ),
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

        egui::Window::new("Delete Padlock")
            .id(egui::Id::new("delete_padlock_confirmation"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!(
                    "Permanently delete Padlock {}?",
                    pending.serial_number,
                ));

                ui.add_space(6.0);

                ui.label("This removes the padlock from inventory.");

                ui.label("This action cannot be undone.");

                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    if ui.button("🔴 Delete Padlock").clicked() {
                        confirm_delete = true;
                    }

                    if ui.button("Cancel").clicked() {
                        cancel_delete = true;
                    }
                });
            });

        if cancel_delete {
            self.pending_delete = None;

            context.set_status_message("Padlock deletion canceled");
        }

        if confirm_delete {
            self.perform_delete(pending, context);
        }
    }

    fn perform_delete(&mut self, pending: PendingDelete, context: &mut AppContext) {
        match context.database().delete_padlock(pending.id) {
            Ok(true) => {
                if self.editing_padlock_id == Some(pending.id) {
                    self.editing_padlock_id = None;

                    self.form = PadlockForm::default();
                }

                self.pending_delete = None;
                self.error_message = None;

                context.mark_data_changed();

                context.set_status_message(format!("Deleted Padlock {}", pending.serial_number,));

                self.refresh(context);
            }

            Ok(false) => {
                self.pending_delete = None;

                self.set_error(
                    context,
                    format!("Padlock {} no longer exists.", pending.serial_number,),
                );
            }

            Err(error) => {
                self.pending_delete = None;

                self.set_error(
                    context,
                    format!(
                        "Unable to delete Padlock {}: {}",
                        pending.serial_number, error,
                    ),
                );
            }
        }
    }

    fn refresh(&mut self, context: &mut AppContext) {
        let padlocks = match context.database().get_all_padlocks() {
            Ok(padlocks) => padlocks,

            Err(error) => {
                self.loaded = true;

                self.set_error(context, format!("Unable to load padlocks: {}", error,));

                return;
            }
        };

        let units = match context.database().get_all_units() {
            Ok(units) => units,

            Err(error) => {
                self.loaded = true;

                self.set_error(
                    context,
                    format!("Unable to load units for padlock assignment: {}", error,),
                );

                return;
            }
        };

        self.padlocks = padlocks;
        self.units = units;
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
