use crate::{
    app::AppContext,
    models::{NewTenant, Tenant},
};

use eframe::egui;

#[derive(Default)]
pub struct TenantsPage {
    form: TenantForm,
    tenants: Vec<Tenant>,
    archived_tenants: Vec<Tenant>,
    loaded: bool,
    editing_tenant_id: Option<i64>,
    pending_archive: Option<PendingArchive>,
    pending_delete: Option<PendingDelete>,
    error_message: Option<String>,
}

#[derive(Debug, Clone)]
struct PendingArchive {
    id: i64,
    display_name: String,
}

#[derive(Debug, Clone)]
struct PendingDelete {
    id: i64,
    display_name: String,
}

struct TenantForm {
    first_name: String,
    last_name: String,
    phone: String,
    email: String,
    active: bool,
    notes: String,
}

impl Default for TenantForm {
    fn default() -> Self {
        Self {
            first_name: String::new(),
            last_name: String::new(),
            phone: String::new(),
            email: String::new(),
            active: true,
            notes: String::new(),
        }
    }
}

impl TenantForm {
    fn from_tenant(tenant: &Tenant) -> Self {
        Self {
            first_name: tenant.first_name.clone(),
            last_name: tenant.last_name.clone(),
            phone: tenant.phone.clone(),
            email: tenant.email.clone(),
            active: tenant.active,
            notes: tenant.notes.clone(),
        }
    }

    fn to_new_tenant(&self) -> Result<NewTenant, String> {
        let first_name = self.first_name.trim().to_string();
        let last_name = self.last_name.trim().to_string();

        if first_name.is_empty() {
            return Err("First name is required.".to_string());
        }

        if last_name.is_empty() {
            return Err("Last name is required.".to_string());
        }

        let email = self.email.trim().to_string();

        if !email.is_empty()
            && (!email.contains('@') || email.starts_with('@') || email.ends_with('@'))
        {
            return Err("Email address is not valid.".to_string());
        }

        Ok(NewTenant {
            first_name,
            last_name,
            phone: self.phone.trim().to_string(),
            email,
            active: self.active,
            notes: self.notes.trim().to_string(),
        })
    }
}

impl TenantsPage {
    pub fn show(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        if !self.loaded {
            self.refresh_tenants(context);
        }

        ui.heading("👤 Tenants");
        ui.separator();

        self.show_tenant_form(ui, context);

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);

        self.show_tenant_list(ui, context);

        ui.add_space(14.0);

        self.show_archived_tenants(ui, context);

        let egui_context = ui.ctx().clone();

        self.show_archive_confirmation(&egui_context, context);
        self.show_delete_confirmation(&egui_context, context);
    }

    fn show_tenant_form(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        if self.editing_tenant_id.is_some() {
            ui.heading("Edit Tenant");
        } else {
            ui.heading("Add Tenant");
        }

        egui::Grid::new("tenant_form")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("First name:");
                ui.text_edit_singleline(&mut self.form.first_name);
                ui.end_row();

                ui.label("Last name:");
                ui.text_edit_singleline(&mut self.form.last_name);
                ui.end_row();

                ui.label("Phone:");
                ui.text_edit_singleline(&mut self.form.phone);
                ui.end_row();

                ui.label("Email:");
                ui.text_edit_singleline(&mut self.form.email);
                ui.end_row();

                ui.label("Active:");
                ui.checkbox(&mut self.form.active, "Available for new rentals");
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
            if self.editing_tenant_id.is_some() {
                if ui.button("Save Changes").clicked() {
                    self.save_changes(context);
                }

                if ui.button("Cancel").clicked() {
                    self.cancel_edit(context);
                }
            } else {
                if ui.button("Add Tenant").clicked() {
                    self.add_tenant(context);
                }

                if ui.button("Clear").clicked() {
                    self.form = TenantForm {
                        active: true,
                        ..Default::default()
                    };
                    self.error_message = None;
                    context.set_status_message("Tenant form cleared");
                }
            }
        });

        if let Some(error_message) = &self.error_message {
            ui.add_space(6.0);
            ui.colored_label(egui::Color32::RED, error_message);
        }
    }

    fn show_tenant_list(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        ui.horizontal(|ui| {
            ui.heading("Current Tenants");

            if ui.button("Refresh").clicked() {
                self.refresh_tenants(context);

                if self.error_message.is_none() {
                    context.set_status_message("Tenant list refreshed");
                }
            }
        });

        if self.tenants.is_empty() {
            ui.label("No current tenants were found.");
            return;
        }

        let mut requested_edit_id: Option<i64> = None;
        let mut requested_archive: Option<PendingArchive> = None;
        let mut requested_delete: Option<PendingDelete> = None;

        egui::ScrollArea::vertical()
            .id_salt("tenant_list_scroll")
            .auto_shrink([false, false])
            .max_height(420.0)
            .show(ui, |ui| {
                egui::Grid::new("tenants_table")
                    .striped(true)
                    .min_col_width(70.0)
                    .spacing([18.0, 6.0])
                    .show(ui, |ui| {
                        ui.strong("Name");
                        ui.strong("Phone");
                        ui.strong("Email");
                        ui.strong("Status");
                        ui.strong("Actions");
                        ui.end_row();

                        for tenant in &self.tenants {
                            let display_name = tenant_display_name(tenant);

                            ui.label(&display_name);
                            ui.label(if tenant.phone.is_empty() {
                                "—"
                            } else {
                                &tenant.phone
                            });
                            ui.label(if tenant.email.is_empty() {
                                "—"
                            } else {
                                &tenant.email
                            });
                            ui.label(if tenant.active { "Active" } else { "Inactive" });

                            ui.horizontal(|ui| {
                                if ui.small_button("✏ Edit").clicked() {
                                    requested_edit_id = Some(tenant.id);
                                }

                                if ui.small_button("📦 Archive").clicked() {
                                    requested_archive = Some(PendingArchive {
                                        id: tenant.id,
                                        display_name: display_name.clone(),
                                    });
                                }

                                if ui.small_button("🗑 Delete").clicked() {
                                    requested_delete = Some(PendingDelete {
                                        id: tenant.id,
                                        display_name,
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

        if let Some(pending) = requested_archive {
            self.pending_archive = Some(pending);
        }

        if let Some(pending) = requested_delete {
            self.pending_delete = Some(pending);
        }
    }

    fn show_archived_tenants(&mut self, ui: &mut egui::Ui, context: &mut AppContext) {
        let title = format!("Archived Tenants ({})", self.archived_tenants.len());

        egui::CollapsingHeader::new(title)
            .default_open(false)
            .show(ui, |ui| {
                if self.archived_tenants.is_empty() {
                    ui.label("No archived tenants were found.");
                    return;
                }

                let mut requested_restore: Option<(i64, String)> = None;

                egui::Grid::new("archived_tenants_table")
                    .striped(true)
                    .min_col_width(70.0)
                    .spacing([18.0, 6.0])
                    .show(ui, |ui| {
                        ui.strong("Name");
                        ui.strong("Phone");
                        ui.strong("Email");
                        ui.strong("Action");
                        ui.end_row();

                        for tenant in &self.archived_tenants {
                            let display_name = tenant_display_name(tenant);

                            ui.label(&display_name);
                            ui.label(if tenant.phone.is_empty() {
                                "—"
                            } else {
                                &tenant.phone
                            });
                            ui.label(if tenant.email.is_empty() {
                                "—"
                            } else {
                                &tenant.email
                            });

                            if ui.small_button("↩ Restore").clicked() {
                                requested_restore = Some((tenant.id, display_name));
                            }

                            ui.end_row();
                        }
                    });

                if let Some((id, display_name)) = requested_restore {
                    self.restore_tenant(id, &display_name, context);
                }
            });
    }

    fn begin_edit(&mut self, id: i64, context: &mut AppContext) {
        let tenant = self.tenants.iter().find(|tenant| tenant.id == id).cloned();

        let Some(tenant) = tenant else {
            self.set_error(context, format!("Unable to find Tenant ID {}.", id));
            return;
        };

        self.form = TenantForm::from_tenant(&tenant);
        self.editing_tenant_id = Some(id);
        self.error_message = None;

        context.set_status_message(format!("Editing {}", tenant_display_name(&tenant)));
    }

    fn cancel_edit(&mut self, context: &mut AppContext) {
        self.editing_tenant_id = None;
        self.form = TenantForm {
            active: true,
            ..Default::default()
        };
        self.error_message = None;
        context.set_status_message("Tenant edit canceled");
    }

    fn add_tenant(&mut self, context: &mut AppContext) {
        self.error_message = None;

        let tenant = match self.form.to_new_tenant() {
            Ok(tenant) => tenant,
            Err(error) => {
                self.set_error(context, error);
                return;
            }
        };

        match context.database().add_tenant(&tenant) {
            Ok(id) => {
                context.mark_data_changed();
                context.set_status_message(format!(
                    "Added {} — database ID {}",
                    new_tenant_display_name(&tenant),
                    id,
                ));

                self.form = TenantForm {
                    active: true,
                    ..Default::default()
                };
                self.refresh_tenants(context);
            }
            Err(error) => {
                self.set_error(
                    context,
                    format!(
                        "Unable to add {}: {}",
                        new_tenant_display_name(&tenant),
                        error,
                    ),
                );
            }
        }
    }

    fn save_changes(&mut self, context: &mut AppContext) {
        self.error_message = None;

        let Some(id) = self.editing_tenant_id else {
            self.set_error(context, "No tenant is selected for editing.");
            return;
        };

        let tenant = match self.form.to_new_tenant() {
            Ok(tenant) => tenant,
            Err(error) => {
                self.set_error(context, error);
                return;
            }
        };

        match context.database().update_tenant(id, &tenant) {
            Ok(true) => {
                context.mark_data_changed();
                context
                    .set_status_message(format!("Updated {}", new_tenant_display_name(&tenant),));

                self.editing_tenant_id = None;
                self.form = TenantForm {
                    active: true,
                    ..Default::default()
                };
                self.refresh_tenants(context);
            }
            Ok(false) => {
                self.set_error(context, format!("Tenant ID {} no longer exists.", id));
            }
            Err(error) => {
                self.set_error(
                    context,
                    format!(
                        "Unable to update {}: {}",
                        new_tenant_display_name(&tenant),
                        error,
                    ),
                );
            }
        }
    }

    fn show_archive_confirmation(&mut self, ctx: &egui::Context, context: &mut AppContext) {
        let Some(pending) = self.pending_archive.clone() else {
            return;
        };

        let mut confirm = false;
        let mut cancel = false;

        egui::Window::new("Archive Tenant")
            .id(egui::Id::new("archive_tenant_confirmation"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!("Archive {}?", pending.display_name));
                ui.add_space(6.0);
                ui.label("The tenant will be hidden from current lists and new rental selections.");
                ui.label("Rental and payment history will be preserved.");
                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    if ui.button("Archive Tenant").clicked() {
                        confirm = true;
                    }

                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });

        if cancel {
            self.pending_archive = None;
            context.set_status_message("Tenant archive canceled");
        }

        if confirm {
            self.perform_archive(pending, context);
        }
    }

    fn perform_archive(&mut self, pending: PendingArchive, context: &mut AppContext) {
        match context.database().archive_tenant(pending.id) {
            Ok(true) => {
                if self.editing_tenant_id == Some(pending.id) {
                    self.editing_tenant_id = None;
                    self.form = TenantForm {
                        active: true,
                        ..Default::default()
                    };
                }

                self.pending_archive = None;
                self.error_message = None;
                context.mark_data_changed();
                context.set_status_message(format!("Archived {}", pending.display_name));
                self.refresh_tenants(context);
            }
            Ok(false) => {
                self.pending_archive = None;
                self.set_error(
                    context,
                    format!(
                        "{} is already archived or no longer exists.",
                        pending.display_name,
                    ),
                );
            }
            Err(error) => {
                self.pending_archive = None;
                self.set_error(
                    context,
                    format!("Unable to archive {}: {}", pending.display_name, error),
                );
            }
        }
    }

    fn restore_tenant(&mut self, id: i64, display_name: &str, context: &mut AppContext) {
        match context.database().restore_tenant(id) {
            Ok(true) => {
                self.error_message = None;
                context.mark_data_changed();
                context.set_status_message(format!("Restored {}", display_name));
                self.refresh_tenants(context);
            }
            Ok(false) => {
                self.set_error(
                    context,
                    format!("{} is not archived or no longer exists.", display_name),
                );
            }
            Err(error) => {
                self.set_error(
                    context,
                    format!("Unable to restore {}: {}", display_name, error),
                );
            }
        }
    }

    fn show_delete_confirmation(&mut self, ctx: &egui::Context, context: &mut AppContext) {
        let Some(pending) = self.pending_delete.clone() else {
            return;
        };

        let mut confirm = false;
        let mut cancel = false;

        egui::Window::new("Delete Tenant")
            .id(egui::Id::new("delete_tenant_confirmation"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!("Permanently delete {}?", pending.display_name));
                ui.add_space(6.0);
                ui.label("Only tenants without rental history can be deleted.");
                ui.label("This action cannot be undone.");
                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    if ui.button("🔴 Delete Tenant").clicked() {
                        confirm = true;
                    }

                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });

        if cancel {
            self.pending_delete = None;
            context.set_status_message("Tenant deletion canceled");
        }

        if confirm {
            self.perform_delete(pending, context);
        }
    }

    fn perform_delete(&mut self, pending: PendingDelete, context: &mut AppContext) {
        match context.database().delete_tenant(pending.id) {
            Ok(true) => {
                if self.editing_tenant_id == Some(pending.id) {
                    self.editing_tenant_id = None;
                    self.form = TenantForm {
                        active: true,
                        ..Default::default()
                    };
                }

                self.pending_delete = None;
                self.error_message = None;
                context.mark_data_changed();
                context.set_status_message(format!("Deleted {}", pending.display_name));
                self.refresh_tenants(context);
            }
            Ok(false) => {
                self.pending_delete = None;
                self.set_error(
                    context,
                    format!("{} no longer exists.", pending.display_name),
                );
            }
            Err(error) => {
                self.pending_delete = None;
                self.set_error(
                    context,
                    format!("Unable to delete {}: {}", pending.display_name, error),
                );
            }
        }
    }

    fn refresh_tenants(&mut self, context: &mut AppContext) {
        let tenants = match context.database().get_all_tenants() {
            Ok(tenants) => tenants,
            Err(error) => {
                self.loaded = true;
                self.set_error(context, format!("Unable to load tenants: {}", error));
                return;
            }
        };

        let archived_tenants = match context.database().get_archived_tenants() {
            Ok(tenants) => tenants,
            Err(error) => {
                self.loaded = true;
                self.set_error(
                    context,
                    format!("Unable to load archived tenants: {}", error),
                );
                return;
            }
        };

        self.tenants = tenants;
        self.archived_tenants = archived_tenants;
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

fn tenant_display_name(tenant: &Tenant) -> String {
    format!("{} {}", tenant.first_name, tenant.last_name)
        .trim()
        .to_string()
}

fn new_tenant_display_name(tenant: &NewTenant) -> String {
    format!("{} {}", tenant.first_name, tenant.last_name)
        .trim()
        .to_string()
}
