use crate::app::AppContext;

use eframe::egui;
use std::{fs, path::PathBuf};

const CONFIG_FILE: &str = "config.toml";
const BACKUP_DIRECTORY: &str = "backups";

#[derive(Default)]
pub struct SettingsPage {
    company_name: String,
    loaded: bool,
    error_message: Option<String>,
    backup_message: Option<String>,
    backup_files: Vec<PathBuf>,
    selected_backup_index: Option<usize>,
    restore_confirmation_open: bool,
}

impl SettingsPage {
    pub fn show(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        if !self.loaded {
            self.load_form(context);
            self.refresh_backup_files();
        }

        ui.heading("⚙ Settings");
        ui.separator();

        ui.add_space(8.0);
        ui.heading("Application Settings");

        egui::Grid::new("application_settings")
            .num_columns(2)
            .spacing([18.0, 10.0])
            .show(ui, |ui| {
                ui.label("Company name:");

                ui.add(egui::TextEdit::singleline(&mut self.company_name).desired_width(300.0));

                ui.end_row();

                ui.label("Database file:");

                ui.monospace(context.config().database_file.as_str());

                ui.end_row();

                ui.label("Application version:");

                ui.label(context.config().version.as_str());

                ui.end_row();

                ui.label("Database status:");

                let spinner_frames = ["|", "/", "-", "\\"];

                let time = ui.ctx().input(|input| input.time);

                let frame = ((time * 8.0) as usize) % spinner_frames.len();

                ui.horizontal(|ui| {
                    ui.monospace(spinner_frames[frame]);

                    ui.colored_label(egui::Color32::GREEN, "Connected");
                });

                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(125));

                ui.end_row();

                ui.label("Configuration file:");

                ui.monospace(CONFIG_FILE);

                ui.end_row();
            });

        ui.add_space(12.0);

        ui.horizontal(|ui| {
            if ui.button("Save Settings").clicked() {
                self.save_settings(context);
            }

            if ui.button("Undo Changes").clicked() {
                self.load_form(context);

                context.set_status_message("Unsaved settings restored");
            }
        });

        if let Some(error_message) = &self.error_message {
            ui.add_space(8.0);

            ui.colored_label(egui::Color32::RED, error_message);
        }

        ui.add_space(20.0);
        ui.separator();
        ui.add_space(10.0);

        ui.heading("Database");

        ui.label("The database file cannot be changed while the application is running.");

        ui.label("To use a different database, close the application and edit config.toml.");

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(10.0);

        self.show_database_backup(ui, context);

        ui.add_space(20.0);
        ui.separator();
        ui.add_space(10.0);

        self.show_database_restore(ui, context);

        let egui_context = ui.ctx().clone();

        self.show_restore_confirmation(&egui_context, context);
    }

    fn load_form(&mut self, context: &AppContext) {
        self.company_name = context.config().company_name.clone();

        self.loaded = true;
        self.error_message = None;
    }

    fn save_settings(&mut self, context: &mut AppContext) {
        self.error_message = None;

        let company_name = self.company_name.trim().to_string();

        if company_name.is_empty() {
            self.set_error(context, "Company name is required.");

            return;
        }

        let previous_company_name = context.config().company_name.clone();

        context.config_mut().company_name = company_name.clone();

        match context.config().save(CONFIG_FILE) {
            Ok(()) => {
                self.company_name = company_name.clone();

                context.set_status_message(format!("Settings saved for {}", company_name,));
            }

            Err(error) => {
                context.config_mut().company_name = previous_company_name;

                self.set_error(context, format!("Unable to save settings: {}", error,));
            }
        }
    }

    fn show_database_backup(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        ui.heading("Database Backup");

        ui.label("Create a verified backup of all units, tenants, rentals, and payments.");

        ui.label("Backups are stored in the application's backups folder.");

        ui.add_space(8.0);

        if ui.button("💾 Create Database Backup").clicked() {
            self.create_database_backup(context);
        }

        if let Some(message) = &self.backup_message {
            ui.add_space(6.0);

            ui.colored_label(egui::Color32::from_rgb(40, 140, 75), message);
        }
    }

    fn create_database_backup(&mut self, context: &mut AppContext) {
        self.backup_message = None;

        match context.database().backup_database(BACKUP_DIRECTORY) {
            Ok(path) => {
                let message = format!("Backup created and verified: {}", path.display(),);

                self.error_message = None;
                self.backup_message = Some(message.clone());

                self.refresh_backup_files();

                context.set_status_message(message);
            }

            Err(error) => {
                let message = format!("Unable to create database backup: {}", error,);

                self.error_message = Some(message.clone());

                context.set_status_message(message);
            }
        }
    }

    fn show_database_restore(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        ui.heading("Restore Database");

        ui.label("Restore units, tenants, rentals, and payments from a verified backup.");

        ui.label("A safety backup of the current database is created before restoration.");

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui.button("Refresh Backup List").clicked() {
                self.refresh_backup_files();

                context.set_status_message("Backup list refreshed");
            }

            ui.label(format!("{} backup(s) found", self.backup_files.len(),));
        });

        ui.add_space(8.0);

        let selected_text = self
            .selected_backup_path()
            .and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "Select a backup".to_string());

        egui::ComboBox::from_id_salt("settings_restore_backup")
            .selected_text(selected_text)
            .width(420.0)
            .show_ui(ui, |ui| {
                for (index, backup_path) in self.backup_files.iter().enumerate() {
                    let filename = backup_path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| backup_path.display().to_string());

                    ui.selectable_value(&mut self.selected_backup_index, Some(index), filename);
                }
            });

        ui.add_space(8.0);

        let restore_enabled = self.selected_backup_path().is_some();

        if ui
            .add_enabled(
                restore_enabled,
                egui::Button::new("↩ Restore Selected Backup"),
            )
            .clicked()
        {
            self.restore_confirmation_open = true;
        }

        if self.backup_files.is_empty() {
            ui.add_space(6.0);

            ui.label("No database backups are available yet.");
        }
    }

    fn refresh_backup_files(&mut self) {
        let previously_selected = self.selected_backup_path();

        let mut backup_files = Vec::new();

        if let Ok(entries) = fs::read_dir(BACKUP_DIRECTORY) {
            for entry in entries.flatten() {
                let path = entry.path();

                let is_database = path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .map(|extension| extension.eq_ignore_ascii_case("db"))
                    .unwrap_or(false);

                if path.is_file() && is_database {
                    backup_files.push(path);
                }
            }
        }

        backup_files.sort_by(|left, right| right.file_name().cmp(&left.file_name()));

        self.backup_files = backup_files;

        self.selected_backup_index = previously_selected
            .and_then(|selected_path| {
                self.backup_files
                    .iter()
                    .position(|path| path == &selected_path)
            })
            .or_else(|| {
                if self.backup_files.is_empty() {
                    None
                } else {
                    Some(0)
                }
            });
    }

    fn selected_backup_path(&self) -> Option<PathBuf> {
        self.selected_backup_index
            .and_then(|index| self.backup_files.get(index).cloned())
    }

    fn show_restore_confirmation(&mut self, ctx: &egui::Context, context: &mut AppContext) {
        if !self.restore_confirmation_open {
            return;
        }

        let Some(backup_path) = self.selected_backup_path() else {
            self.restore_confirmation_open = false;

            self.set_error(context, "No database backup is selected.");

            return;
        };

        let backup_name = backup_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| backup_path.display().to_string());

        let mut confirm_restore = false;
        let mut cancel_restore = false;

        egui::Window::new("Confirm Database Restore")
            .id(egui::Id::new("database_restore_confirmation"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.strong(format!("Restore {}?", backup_name,));

                ui.add_space(8.0);

                ui.label("The current database will be replaced with the selected backup.");

                ui.label("A safety backup of the current database will be created first.");

                ui.label("The application will close after a successful restore.");

                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    if ui.button("🔴 Restore and Exit").clicked() {
                        confirm_restore = true;
                    }

                    if ui.button("Cancel").clicked() {
                        cancel_restore = true;
                    }
                });
            });

        if cancel_restore {
            self.restore_confirmation_open = false;

            context.set_status_message("Database restore canceled");
        }

        if confirm_restore {
            self.restore_confirmation_open = false;

            match context
                .database_mut()
                .restore_database(&backup_path, BACKUP_DIRECTORY)
            {
                Ok(safety_backup_path) => {
                    context.mark_data_changed();

                    context.set_status_message(format!(
                        "Database restored. Safety backup: {}",
                        safety_backup_path.display(),
                    ));

                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }

                Err(error) => {
                    self.set_error(context, format!("Unable to restore database: {}", error,));
                }
            }
        }
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
