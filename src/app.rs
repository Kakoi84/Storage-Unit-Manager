use crate::{config::Config, database::Database, page::Page, pages, widgets};

use eframe::egui;

/// Everything the application needs that should be shared.
pub struct AppContext {
    config: Config,
    database: Database,
    status_message: String,
    data_revision: u64,
}

impl AppContext {
    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    pub fn database(&self) -> &Database {
        &self.database
    }

    pub fn database_mut(&mut self) -> &mut Database {
        &mut self.database
    }

    pub fn status_message(&self) -> &str {
        &self.status_message
    }

    pub fn set_status_message<S>(&mut self, message: S)
    where
        S: Into<String>,
    {
        self.status_message = message.into();
    }

    pub fn data_revision(&self) -> u64 {
        self.data_revision
    }

    pub fn mark_data_changed(&mut self) {
        self.data_revision = self.data_revision.wrapping_add(1);
    }
}

/// Main application object.
pub struct StorageManager {
    context: AppContext,
    current_page: Page,
    dashboard_page: pages::dashboard::DashboardPage,
    units_page: pages::units::UnitsPage,
    tenants_page: pages::tenants::TenantsPage,
    rentals_page: pages::rentals::RentalsPage,
    payments_page: pages::payments::PaymentsPage,
    padlocks_page: pages::padlocks::PadlocksPage,
    reports_page: pages::reports::ReportsPage,
    settings_page: pages::settings::SettingsPage,
}

impl StorageManager {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::configure_theme(&cc.egui_ctx);

        let config = Config::load_or_create("config.toml")
            .expect("🔴 Unable to load application configuration.");

        let database = Database::new(&config.database_file)
            .expect("🔴 Unable to open StorageManager database.");

        database
            .health_check()
            .expect("🔴 StorageManager database health check failed.");

        Self {
            context: AppContext {
                config,
                database,
                status_message: "🟢 Database Connected".to_string(),
                data_revision: 0,
            },

            current_page: Page::Dashboard,
            dashboard_page: pages::dashboard::DashboardPage::default(),
            units_page: pages::units::UnitsPage::default(),
            tenants_page: pages::tenants::TenantsPage::default(),
            rentals_page: pages::rentals::RentalsPage::default(),
            payments_page: pages::payments::PaymentsPage::default(),
            padlocks_page: pages::padlocks::PadlocksPage::default(),
            reports_page: pages::reports::ReportsPage::default(),
            settings_page: pages::settings::SettingsPage::default(),
        }
    }

    fn configure_theme(ctx: &egui::Context) {
        // We will move this to theme.rs later.
        ctx.set_visuals(egui::Visuals::dark());
    }
}

impl eframe::App for StorageManager {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        //---------------------------------------------------------
        // Navigation
        //---------------------------------------------------------

        egui::SidePanel::left("navigation")
            .default_width(200.0)
            .show(ctx, |ui| {
                widgets::navigation::show(ui, &mut self.current_page, self.context.config());
            });

        //---------------------------------------------------------
        // Status Bar
        //---------------------------------------------------------

        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(24.0)
            .show(ctx, |ui| {
                widgets::statusbar::show(ui, self.context.config(), self.context.status_message(), self.context.database());
            });

        //---------------------------------------------------------
        // Main Window
        //---------------------------------------------------------

        egui::CentralPanel::default().show(ctx, |ui| match self.current_page {
            Page::Dashboard => {
                self.dashboard_page.show(ui, &mut self.context);
            }

            Page::Units => {
                self.units_page.show(ui, &mut self.context);
            }

            Page::Tenants => {
                self.tenants_page.show(ui, &mut self.context);
            }

            Page::Payments => {
                self.payments_page.show(ui, &mut self.context);
            }

            Page::Padlocks => {
                self.padlocks_page.show(ui, &mut self.context);
            }

            Page::Reports => {
                self.reports_page.show(ui, &mut self.context);
            }

            Page::Settings => {
                self.settings_page.show(ui, &mut self.context);
            }

            Page::Rentals => {
                self.rentals_page.show(ui, &mut self.context);
            }
        });
    }
}
