use crate::models::{
    NewPadlock, NewPayment, NewRental, NewTenant, NewUnit, Padlock, Payment, PaymentMethod, Rental,
    ReportSummary, Tenant, Unit,
};

use anyhow::{Result, anyhow};
use rusqlite::{Connection, OptionalExtension, backup::Backup, params};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

/// Owns the application's SQLite connection.
pub struct Database {
    connection: Connection,
}

impl Database {
    /// Opens the SQLite database and creates or upgrades the schema.
    pub fn new<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let connection = Connection::open(path)?;
        let database = Self { connection };

        database.initialize()?;
        database.ensure_units_archived_column()?;
        database.ensure_units_needs_cleaned_column()?;
        database.ensure_units_lock_columns()?;
        database.ensure_units_layout_columns()?;
        database.ensure_tenants_archived_column()?;
        database.reconcile_unit_occupancy()?;

        Ok(database)
    }

    /// Creates all tables and indexes required by the application.
    fn initialize(&self) -> Result<()> {
        self.connection.execute_batch(
            "
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS units (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                unit_number TEXT NOT NULL UNIQUE,
                building TEXT NOT NULL DEFAULT '',
                width REAL,
                length REAL,
                monthly_rent_cents INTEGER NOT NULL DEFAULT 0
                CHECK(monthly_rent_cents >= 0),
                occupied INTEGER NOT NULL DEFAULT 0
                CHECK(occupied IN (0, 1)),

                needs_cleaned INTEGER NOT NULL DEFAULT 0
                CHECK(needs_cleaned IN (0, 1)),
                yellow_lock INTEGER NOT NULL DEFAULT 0
                CHECK(yellow_lock IN (0, 1)),
                red_lock INTEGER NOT NULL DEFAULT 0
                CHECK(red_lock IN (0, 1)),
                layout_section TEXT NOT NULL DEFAULT '',
                display_order INTEGER NOT NULL DEFAULT 0
                CHECK(display_order >= 0),
                archived INTEGER NOT NULL DEFAULT 0
                CHECK(archived IN (0, 1)),
                notes TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS tenants (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            first_name TEXT NOT NULL,
            last_name TEXT NOT NULL,
            phone TEXT NOT NULL DEFAULT '',
            email TEXT NOT NULL DEFAULT '',
            active INTEGER NOT NULL DEFAULT 1
            CHECK(active IN (0, 1)),
                                      archived INTEGER NOT NULL DEFAULT 0
                                      CHECK(archived IN (0, 1)),
                                      notes TEXT NOT NULL DEFAULT '',
                                      created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                                      updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE INDEX IF NOT EXISTS idx_tenants_name
        ON tenants(last_name, first_name);

        CREATE TABLE IF NOT EXISTS rentals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tenant_id INTEGER NOT NULL,
            unit_id INTEGER NOT NULL,
            start_date TEXT NOT NULL,
            end_date TEXT,
            monthly_rent_cents INTEGER NOT NULL
            CHECK(monthly_rent_cents >= 0),
                                      notes TEXT NOT NULL DEFAULT '',
                                      created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                                      updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                                      FOREIGN KEY(tenant_id)
        REFERENCES tenants(id)
        ON DELETE RESTRICT,
        FOREIGN KEY(unit_id)
        REFERENCES units(id)
        ON DELETE RESTRICT
        );

        CREATE INDEX IF NOT EXISTS idx_rentals_tenant
        ON rentals(tenant_id);

        CREATE INDEX IF NOT EXISTS idx_rentals_unit
        ON rentals(unit_id);

        CREATE UNIQUE INDEX IF NOT EXISTS idx_rentals_active_unit
        ON rentals(unit_id)
        WHERE end_date IS NULL;

        CREATE TABLE IF NOT EXISTS payments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            rental_id INTEGER NOT NULL,
            payment_date TEXT NOT NULL,
            amount_cents INTEGER NOT NULL
            CHECK(amount_cents > 0),
                                      payment_method TEXT NOT NULL DEFAULT 'Cash',
                                      reference TEXT NOT NULL DEFAULT '',
                                      notes TEXT NOT NULL DEFAULT '',
                                      created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                                      FOREIGN KEY(rental_id)
        REFERENCES rentals(id)
        ON DELETE RESTRICT
        );

        CREATE INDEX IF NOT EXISTS idx_payments_rental
        ON payments(rental_id);

        CREATE INDEX IF NOT EXISTS idx_payments_date
        ON payments(payment_date);

        CREATE TABLE IF NOT EXISTS padlocks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            serial_number TEXT NOT NULL UNIQUE,
            combination TEXT NOT NULL,
            unit_id INTEGER UNIQUE,
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY(unit_id)
        REFERENCES units(id)
        ON DELETE SET NULL
        );

        CREATE INDEX IF NOT EXISTS idx_padlocks_serial_number
        ON padlocks(serial_number);

        CREATE INDEX IF NOT EXISTS idx_padlocks_unit_id
        ON padlocks(unit_id);
        ",
        )?;

        Ok(())
    }

    /// Verifies that SQLite is responding.
    pub fn health_check(&self) -> Result<()> {
        self.connection.query_row("SELECT 1;", [], |_row| Ok(()))?;

        Ok(())
    }

    // ---------------------------------------------------------------------
    // Units
    // ---------------------------------------------------------------------

    pub fn add_unit(&self, unit: &NewUnit) -> Result<i64> {
        self.connection.execute(
            "
            INSERT INTO units (
                unit_number,
                building,
                width,
                length,
                monthly_rent_cents,
                needs_cleaned,
                occupied,
                yellow_lock,
                red_lock,
                layout_section,
                display_order,
                notes
        )
        VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, ?6, ?7, ?8, ?9, ?10);
        ",
            params![
                &unit.unit_number,
                &unit.building,
                unit.width,
                unit.length,
                unit.monthly_rent_cents,
                unit.yellow_lock as i64,
                unit.red_lock as i64,
                &unit.layout_section,
                unit.display_order,
                &unit.notes,
            ],
        )?;

        Ok(self.connection.last_insert_rowid())
    }

    pub fn get_all_units(&self) -> Result<Vec<Unit>> {
        let mut statement = self.connection.prepare(
            "
            SELECT
            id,
            unit_number,
            building,
            width,
            length,
            monthly_rent_cents,
            occupied,
            needs_cleaned,
            yellow_lock,
            red_lock,
            layout_section,
            display_order,
            notes
            FROM units
            WHERE archived = 0
            ORDER BY
            building COLLATE NOCASE,
            layout_section COLLATE NOCASE,
            display_order,
            unit_number COLLATE NOCASE;
            ",
        )?;

        let rows = statement.query_map([], map_unit)?;
        let units = rows.collect::<rusqlite::Result<Vec<Unit>>>()?;

        Ok(units)
    }

    pub fn get_vacant_units(&self) -> Result<Vec<Unit>> {
        let mut statement = self.connection.prepare(
            "
            SELECT
            units.id,
            units.unit_number,
            units.building,
            units.width,
            units.length,
            units.monthly_rent_cents,
            units.occupied,
            units.needs_cleaned,
            units.yellow_lock,
            units.red_lock,
            units.layout_section,
            units.display_order,
            units.notes
            FROM units
            WHERE
            units.archived = 0
            AND units.occupied = 0
            AND units.needs_cleaned = 0
            AND NOT EXISTS (
                SELECT 1
                FROM rentals
                WHERE
                rentals.unit_id = units.id
                AND rentals.end_date IS NULL
        )
        ORDER BY
        units.building COLLATE NOCASE,
        units.layout_section COLLATE NOCASE,
        units.display_order,
        units.unit_number COLLATE NOCASE;
        ",
        )?;

        let rows = statement.query_map([], map_unit)?;
        let units = rows.collect::<rusqlite::Result<Vec<Unit>>>()?;

        Ok(units)
    }

    pub fn get_archived_units(&self) -> Result<Vec<Unit>> {
        let mut statement = self.connection.prepare(
            "
            SELECT
            id,
            unit_number,
            building,
            width,
            length,
            monthly_rent_cents,
            occupied,
            needs_cleaned,
            yellow_lock,
            red_lock,
            layout_section,
            display_order,
            notes
            FROM units
            WHERE archived = 1
            ORDER BY
            building COLLATE NOCASE,
            layout_section COLLATE NOCASE,
            display_order,
            unit_number COLLATE NOCASE;
            ",
        )?;

        let rows = statement.query_map([], map_unit)?;
        let units = rows.collect::<rusqlite::Result<Vec<Unit>>>()?;

        Ok(units)
    }

    pub fn update_unit(&self, id: i64, unit: &NewUnit) -> Result<bool> {
        let affected_rows = self.connection.execute(
            "
            UPDATE units
            SET
            unit_number = ?1,
            building = ?2,
            width = ?3,
            length = ?4,
            monthly_rent_cents = ?5,
            yellow_lock = ?6,
            red_lock = ?7,
            layout_section = ?8,
            display_order = ?9,
            notes = ?10,
            updated_at = CURRENT_TIMESTAMP
            WHERE
            id = ?11
            AND archived = 0;
            ",
            params![
                &unit.unit_number,
                &unit.building,
                unit.width,
                unit.length,
                unit.monthly_rent_cents,
                unit.yellow_lock as i64,
                unit.red_lock as i64,
                &unit.layout_section,
                unit.display_order,
                &unit.notes,
                id,
            ],
        )?;

        Ok(affected_rows == 1)
    }

    pub fn delete_unit(&self, id: i64) -> Result<bool> {
        let rental_count: i64 = self.connection.query_row(
            "
            SELECT COUNT(*)
        FROM rentals
        WHERE unit_id = ?1;
        ",
            params![id],
            |row| row.get(0),
        )?;

        if rental_count > 0 {
            return Err(anyhow!(
                "This unit has rental history and cannot be permanently deleted. Archive the unit instead."
            ));
        }

        let affected_rows = self
            .connection
            .execute("DELETE FROM units WHERE id = ?1;", params![id])?;

        Ok(affected_rows == 1)
    }

    pub fn archive_unit(&self, id: i64) -> Result<bool> {
        let active_rentals: i64 = self.connection.query_row(
            "
            SELECT COUNT(*)
        FROM rentals
        WHERE
        unit_id = ?1
        AND end_date IS NULL;
        ",
            params![id],
            |row| row.get(0),
        )?;

        if active_rentals > 0 {
            return Err(anyhow!(
                "This unit has an active rental. End the rental before archiving the unit."
            ));
        }

        let affected_rows = self.connection.execute(
            "
            UPDATE units
            SET
            archived = 1,
            occupied = 0,
            updated_at = CURRENT_TIMESTAMP
            WHERE
            id = ?1
            AND archived = 0;
            ",
            params![id],
        )?;

        Ok(affected_rows == 1)
    }

    pub fn restore_unit(&self, id: i64) -> Result<bool> {
        let affected_rows = self.connection.execute(
            "
            UPDATE units
            SET
            archived = 0,
            updated_at = CURRENT_TIMESTAMP
            WHERE
            id = ?1
            AND archived = 1;
            ",
            params![id],
        )?;

        Ok(affected_rows == 1)
    }

    // ---------------------------------------------------------------------
    // Tenants
    // ---------------------------------------------------------------------

    pub fn add_tenant(&self, tenant: &NewTenant) -> Result<i64> {
        self.connection.execute(
            "
            INSERT INTO tenants (
                first_name,
                last_name,
                phone,
                email,
                active,
                notes
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6);
        ",
            params![
                &tenant.first_name,
                &tenant.last_name,
                &tenant.phone,
                &tenant.email,
                tenant.active as i64,
                &tenant.notes,
            ],
        )?;

        Ok(self.connection.last_insert_rowid())
    }

    pub fn get_all_tenants(&self) -> Result<Vec<Tenant>> {
        self.get_tenants_by_archive_status(false, false)
    }

    pub fn get_active_tenants(&self) -> Result<Vec<Tenant>> {
        self.get_tenants_by_archive_status(false, true)
    }

    pub fn get_archived_tenants(&self) -> Result<Vec<Tenant>> {
        self.get_tenants_by_archive_status(true, false)
    }

    fn get_tenants_by_archive_status(
        &self,
        archived: bool,
        active_only: bool,
    ) -> Result<Vec<Tenant>> {
        let sql = if active_only {
            "
            SELECT
            id,
            first_name,
            last_name,
            phone,
            email,
            active,
            notes
            FROM tenants
            WHERE
            archived = ?1
            AND active = 1
            ORDER BY
            last_name COLLATE NOCASE,
            first_name COLLATE NOCASE;
            "
        } else {
            "
            SELECT
            id,
            first_name,
            last_name,
            phone,
            email,
            active,
            notes
            FROM tenants
            WHERE archived = ?1
            ORDER BY
            last_name COLLATE NOCASE,
            first_name COLLATE NOCASE;
            "
        };

        let mut statement = self.connection.prepare(sql)?;
        let rows = statement.query_map(params![archived as i64], map_tenant)?;
        let tenants = rows.collect::<rusqlite::Result<Vec<Tenant>>>()?;

        Ok(tenants)
    }

    pub fn update_tenant(&self, id: i64, tenant: &NewTenant) -> Result<bool> {
        let affected_rows = self.connection.execute(
            "
            UPDATE tenants
            SET
            first_name = ?1,
            last_name = ?2,
            phone = ?3,
            email = ?4,
            active = ?5,
            notes = ?6,
            updated_at = CURRENT_TIMESTAMP
            WHERE
            id = ?7
            AND archived = 0;
            ",
            params![
                &tenant.first_name,
                &tenant.last_name,
                &tenant.phone,
                &tenant.email,
                tenant.active as i64,
                &tenant.notes,
                id,
            ],
        )?;

        Ok(affected_rows == 1)
    }

    pub fn delete_tenant(&self, id: i64) -> Result<bool> {
        let rental_count: i64 = self.connection.query_row(
            "
            SELECT COUNT(*)
        FROM rentals
        WHERE tenant_id = ?1;
        ",
            params![id],
            |row| row.get(0),
        )?;

        if rental_count > 0 {
            return Err(anyhow!(
                "This tenant has rental history and cannot be permanently deleted. Archive the tenant instead."
            ));
        }

        let affected_rows = self
            .connection
            .execute("DELETE FROM tenants WHERE id = ?1;", params![id])?;

        Ok(affected_rows == 1)
    }

    pub fn archive_tenant(&self, id: i64) -> Result<bool> {
        let active_rentals: i64 = self.connection.query_row(
            "
            SELECT COUNT(*)
        FROM rentals
        WHERE
        tenant_id = ?1
        AND end_date IS NULL;
        ",
            params![id],
            |row| row.get(0),
        )?;

        if active_rentals > 0 {
            return Err(anyhow!(
                "This tenant has an active rental. End the rental before archiving the tenant."
            ));
        }

        let affected_rows = self.connection.execute(
            "
            UPDATE tenants
            SET
            archived = 1,
            updated_at = CURRENT_TIMESTAMP
            WHERE
            id = ?1
            AND archived = 0;
            ",
            params![id],
        )?;

        Ok(affected_rows == 1)
    }

    pub fn restore_tenant(&self, id: i64) -> Result<bool> {
        let affected_rows = self.connection.execute(
            "
            UPDATE tenants
            SET
            archived = 0,
            updated_at = CURRENT_TIMESTAMP
            WHERE
            id = ?1
            AND archived = 1;
            ",
            params![id],
        )?;

        Ok(affected_rows == 1)
    }

    // ---------------------------------------------------------------------
    // Rentals
    // ---------------------------------------------------------------------

    pub fn start_rental(&mut self, rental: &NewRental) -> Result<i64> {
        if rental.tenant_id <= 0 {
            return Err(anyhow!("A tenant must be selected."));
        }

        if rental.unit_id <= 0 {
            return Err(anyhow!("A storage unit must be selected."));
        }

        if rental.start_date.trim().is_empty() {
            return Err(anyhow!("A rental start date is required."));
        }

        if rental.monthly_rent_cents < 0 {
            return Err(anyhow!("Monthly rent cannot be negative."));
        }

        let transaction = self.connection.transaction()?;

        let active_tenant_count: i64 = transaction.query_row(
            "
            SELECT COUNT(*)
        FROM tenants
        WHERE
        id = ?1
        AND active = 1
        AND archived = 0;
        ",
            params![rental.tenant_id],
            |row| row.get(0),
        )?;

        if active_tenant_count != 1 {
            return Err(anyhow!(
                "The selected tenant does not exist, is inactive, or is archived."
            ));
        }

        let affected_units = transaction.execute(
            "
            UPDATE units
            SET
            occupied = 1,
            needs_cleaned = 0,
            updated_at = CURRENT_TIMESTAMP
            WHERE
            id = ?1
            AND occupied = 0
            AND archived = 0
            AND needs_cleaned = 0;
            ",
            params![rental.unit_id],
        )?;

        if affected_units != 1 {
            return Err(anyhow!("The selected storage unit is not available."));
        }

        transaction.execute(
            "
            INSERT INTO rentals (
                tenant_id,
                unit_id,
                start_date,
                end_date,
                monthly_rent_cents,
                notes
        )
        VALUES (?1, ?2, ?3, NULL, ?4, ?5);
        ",
            params![
                rental.tenant_id,
                rental.unit_id,
                rental.start_date.trim(),
                rental.monthly_rent_cents,
                rental.notes.trim(),
            ],
        )?;

        let rental_id = transaction.last_insert_rowid();
        transaction.commit()?;

        Ok(rental_id)
    }

    pub fn get_active_rentals(&self) -> Result<Vec<Rental>> {
        let mut statement = self.connection.prepare(
            "
            SELECT
            rentals.id,
            tenants.first_name || ' ' || tenants.last_name AS tenant_name,
            units.unit_number,
            rentals.start_date,
            rentals.monthly_rent_cents
            FROM rentals
            INNER JOIN tenants
            ON tenants.id = rentals.tenant_id
            INNER JOIN units
            ON units.id = rentals.unit_id
            WHERE rentals.end_date IS NULL
            ORDER BY units.unit_number COLLATE NOCASE;
            ",
        )?;

        let rows = statement.query_map([], |row| {
            Ok(Rental {
                id: row.get(0)?,
                tenant_name: row.get(1)?,
                unit_number: row.get(2)?,
                start_date: row.get(3)?,
                monthly_rent_cents: row.get(4)?,
            })
        })?;

        let rentals = rows.collect::<rusqlite::Result<Vec<Rental>>>()?;

        Ok(rentals)
    }

    pub fn update_active_rental_rent(
        &self,
        rental_id: i64,
        monthly_rent_cents: i64,
    ) -> Result<bool> {
        if rental_id <= 0 {
            return Err(anyhow!("A valid rental must be selected."));
        }

        if monthly_rent_cents < 0 {
            return Err(anyhow!("Monthly rent cannot be negative."));
        }

        let affected_rows = self.connection.execute(
            "
            UPDATE rentals
            SET
                monthly_rent_cents = ?1,
                updated_at = CURRENT_TIMESTAMP
            WHERE
                id = ?2
                AND end_date IS NULL;
            ",
            params![monthly_rent_cents, rental_id],
        )?;

        Ok(affected_rows == 1)
    }

    pub fn end_rental(&mut self, rental_id: i64, end_date: &str) -> Result<bool> {
        let end_date = end_date.trim();

        if end_date.is_empty() {
            return Err(anyhow!("A rental end date is required."));
        }

        let transaction = self.connection.transaction()?;

        let unit_id: Option<i64> = transaction
            .query_row(
                "
            SELECT unit_id
            FROM rentals
            WHERE
            id = ?1
            AND end_date IS NULL;
            ",
                params![rental_id],
                |row| row.get(0),
            )
            .optional()?;

        let Some(unit_id) = unit_id else {
            return Ok(false);
        };

        let affected_rentals = transaction.execute(
            "
            UPDATE rentals
            SET
            end_date = ?1,
            updated_at = CURRENT_TIMESTAMP
            WHERE
            id = ?2
            AND end_date IS NULL;
            ",
            params![end_date, rental_id],
        )?;

        if affected_rentals != 1 {
            return Ok(false);
        }

        transaction.execute(
            "
            UPDATE units
            SET
            occupied = 0,
            needs_cleaned = 1,
            updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1;
            ",
            params![unit_id],
        )?;

        transaction.commit()?;

        Ok(true)
    }

    // ---------------------------------------------------------------------
    // Payments and reports
    // ---------------------------------------------------------------------

    pub fn record_payment(&self, payment: &NewPayment) -> Result<i64> {
        if payment.rental_id <= 0 {
            return Err(anyhow!("A rental must be selected."));
        }

        if payment.payment_date.trim().is_empty() {
            return Err(anyhow!("A payment date is required."));
        }

        if payment.amount_cents <= 0 {
            return Err(anyhow!("Payment amount must be greater than zero."));
        }

        let rental_count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM rentals WHERE id = ?1;",
            params![payment.rental_id],
            |row| row.get(0),
        )?;

        if rental_count != 1 {
            return Err(anyhow!("The selected rental does not exist."));
        }

        self.connection.execute(
            "
            INSERT INTO payments (
                rental_id,
                payment_date,
                amount_cents,
                payment_method,
                reference,
                notes
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6);
        ",
            params![
                payment.rental_id,
                payment.payment_date.trim(),
                payment.amount_cents,
                payment.payment_method.label(),
                payment.reference.trim(),
                payment.notes.trim(),
            ],
        )?;

        Ok(self.connection.last_insert_rowid())
    }

    pub fn get_all_payments(&self) -> Result<Vec<Payment>> {
        let mut statement = self.connection.prepare(
            "
            SELECT
            tenants.first_name || ' ' || tenants.last_name AS tenant_name,
            units.unit_number,
            payments.payment_date,
            payments.amount_cents,
            payments.payment_method,
            payments.reference
            FROM payments
            INNER JOIN rentals
            ON rentals.id = payments.rental_id
            INNER JOIN tenants
            ON tenants.id = rentals.tenant_id
            INNER JOIN units
            ON units.id = rentals.unit_id
            ORDER BY
            payments.payment_date DESC,
            payments.id DESC;
            ",
        )?;

        let rows = statement.query_map([], |row| {
            let payment_method: String = row.get(4)?;

            Ok(Payment {
                tenant_name: row.get(0)?,
                unit_number: row.get(1)?,
                payment_date: row.get(2)?,
                amount_cents: row.get(3)?,
                payment_method: PaymentMethod::from_database(&payment_method),
                reference: row.get(5)?,
            })
        })?;

        let payments = rows.collect::<rusqlite::Result<Vec<Payment>>>()?;

        Ok(payments)
    }

    pub fn get_report_summary(&self) -> Result<ReportSummary> {
        let summary = self.connection.query_row(
            "
            SELECT
                (
                    SELECT COUNT(*)
                    FROM units
                    WHERE archived = 0
                ) AS total_units,

                (
                    SELECT COUNT(*)
                    FROM units
                    WHERE
                        archived = 0
                        AND occupied = 1
                ) AS occupied_units,

                (
                    SELECT COUNT(*)
                    FROM units
                    WHERE
                        archived = 0
                        AND occupied = 0
                        AND needs_cleaned = 0
                ) AS vacant_units,

                (
                    SELECT COUNT(*)
                    FROM units
                    WHERE
                        archived = 0
                        AND occupied = 0
                        AND needs_cleaned = 1
                ) AS needs_cleaned_units,

                (
                    SELECT COUNT(*)
                    FROM tenants
                    WHERE
                        active = 1
                        AND archived = 0
                ) AS active_tenants,

                (
                    SELECT COUNT(*)
                    FROM rentals
                    WHERE end_date IS NULL
                ) AS active_rentals,

                (
                    SELECT COALESCE(
                        SUM(monthly_rent_cents),
                        0
                    )
                    FROM rentals
                    WHERE end_date IS NULL
                ) AS monthly_rent_cents,

                (
                    SELECT COALESCE(
                        SUM(amount_cents),
                        0
                    )
                    FROM payments
                ) AS total_payments_cents;
            ",
            [],
            |row| {
                Ok(ReportSummary {
                    total_units: row.get(0)?,
                    occupied_units: row.get(1)?,
                    vacant_units: row.get(2)?,
                    needs_cleaned_units: row.get(3)?,
                    active_tenants: row.get(4)?,
                    active_rentals: row.get(5)?,
                    monthly_rent_cents: row.get(6)?,
                    total_payments_cents: row.get(7)?,
                })
            },
        )?;

        Ok(summary)
    }

    // ---------------------------------------------------------------------
    // Padlocks
    // ---------------------------------------------------------------------

    pub fn add_padlock(&self, padlock: &NewPadlock) -> Result<i64> {
        self.ensure_padlock_unit_available(None, padlock.unit_id)?;

        self.connection.execute(
            "
            INSERT INTO padlocks (
                serial_number,
                combination,
                unit_id,
                notes
        )
        VALUES (?1, ?2, ?3, ?4);
        ",
            params![
                &padlock.serial_number,
                &padlock.combination,
                padlock.unit_id,
                &padlock.notes,
            ],
        )?;

        Ok(self.connection.last_insert_rowid())
    }

    pub fn get_all_padlocks(&self) -> Result<Vec<Padlock>> {
        let mut statement = self.connection.prepare(
            "
            SELECT
            padlocks.id,
            padlocks.serial_number,
            padlocks.combination,
            padlocks.unit_id,
            units.unit_number,
            padlocks.notes
            FROM padlocks
            LEFT JOIN units
            ON units.id = padlocks.unit_id
            ORDER BY
            CASE
            WHEN padlocks.unit_id IS NULL THEN 0
            ELSE 1
            END,
            padlocks.serial_number COLLATE NOCASE;
            ",
        )?;

        let rows = statement.query_map([], |row| {
            Ok(Padlock {
                id: row.get(0)?,
                serial_number: row.get(1)?,
                combination: row.get(2)?,
                unit_id: row.get(3)?,
                unit_number: row.get(4)?,
                notes: row.get(5)?,
            })
        })?;

        let padlocks = rows.collect::<rusqlite::Result<Vec<Padlock>>>()?;

        Ok(padlocks)
    }

    pub fn update_padlock(&self, id: i64, padlock: &NewPadlock) -> Result<bool> {
        self.ensure_padlock_unit_available(Some(id), padlock.unit_id)?;

        let affected_rows = self.connection.execute(
            "
            UPDATE padlocks
            SET
            serial_number = ?1,
            combination = ?2,
            unit_id = ?3,
            notes = ?4,
            updated_at = CURRENT_TIMESTAMP
            WHERE id = ?5;
            ",
            params![
                &padlock.serial_number,
                &padlock.combination,
                padlock.unit_id,
                &padlock.notes,
                id,
            ],
        )?;

        Ok(affected_rows == 1)
    }

    pub fn delete_padlock(&self, id: i64) -> Result<bool> {
        let affected_rows = self
            .connection
            .execute("DELETE FROM padlocks WHERE id = ?1;", params![id])?;

        Ok(affected_rows == 1)
    }

    fn ensure_padlock_unit_available(
        &self,
        current_padlock_id: Option<i64>,
        unit_id: Option<i64>,
    ) -> Result<()> {
        let Some(unit_id) = unit_id else {
            return Ok(());
        };

        let unit_is_active: i64 = self.connection.query_row(
            "
            SELECT COUNT(*)
        FROM units
        WHERE
        id = ?1
        AND archived = 0;
        ",
            params![unit_id],
            |row| row.get(0),
        )?;

        if unit_is_active != 1 {
            return Err(anyhow!("The selected unit does not exist or is archived."));
        }

        let existing_lock: Option<String> = match current_padlock_id {
            Some(padlock_id) => self
                .connection
                .query_row(
                    "
                SELECT serial_number
                FROM padlocks
                WHERE
                unit_id = ?1
                AND id != ?2;
                ",
                    params![unit_id, padlock_id],
                    |row| row.get(0),
                )
                .optional()?,
            None => self
                .connection
                .query_row(
                    "
                SELECT serial_number
                FROM padlocks
                WHERE unit_id = ?1;
                ",
                    params![unit_id],
                    |row| row.get(0),
                )
                .optional()?,
        };

        if let Some(serial_number) = existing_lock {
            return Err(anyhow!(
                "The selected unit already has Padlock {} assigned to it.",
                serial_number
            ));
        }

        Ok(())
    }

    // ---------------------------------------------------------------------
    // Maintenance, migration, backup, and restore
    // ---------------------------------------------------------------------

    pub fn reconcile_unit_occupancy(&self) -> Result<usize> {
        let affected_rows = self.connection.execute(
            "
            UPDATE units
            SET
                occupied = CASE
                    WHEN EXISTS (
                        SELECT 1
                        FROM rentals
                        WHERE
                            rentals.unit_id = units.id
                            AND rentals.end_date IS NULL
                    )
                    THEN 1
                    ELSE 0
                END,

                needs_cleaned = CASE
                    WHEN EXISTS (
                        SELECT 1
                        FROM rentals
                        WHERE
                            rentals.unit_id = units.id
                            AND rentals.end_date IS NULL
                    )
                    THEN 0
                    ELSE needs_cleaned
                END,

                updated_at = CURRENT_TIMESTAMP

            WHERE
                occupied != CASE
                    WHEN EXISTS (
                        SELECT 1
                        FROM rentals
                        WHERE
                            rentals.unit_id = units.id
                            AND rentals.end_date IS NULL
                    )
                    THEN 1
                    ELSE 0
                END

                OR (
                    needs_cleaned != 0
                    AND EXISTS (
                        SELECT 1
                        FROM rentals
                        WHERE
                            rentals.unit_id = units.id
                            AND rentals.end_date IS NULL
                    )
                );
            ",
            [],
        )?;

        Ok(affected_rows)
    }

    fn ensure_units_archived_column(&self) -> Result<()> {
        self.ensure_boolean_column("units", "archived")?;

        self.connection.execute(
            "
            CREATE INDEX IF NOT EXISTS idx_units_archived
            ON units(archived);
        ",
            [],
        )?;

        Ok(())
    }

    fn ensure_tenants_archived_column(&self) -> Result<()> {
        self.ensure_boolean_column("tenants", "archived")?;

        self.connection.execute(
            "
            CREATE INDEX IF NOT EXISTS idx_tenants_archived
            ON tenants(archived);
        ",
            [],
        )?;

        Ok(())
    }

    fn ensure_boolean_column(&self, table: &str, column: &str) -> Result<()> {
        let pragma = format!("PRAGMA table_info({table});");
        let mut statement = self.connection.prepare(&pragma)?;
        let columns = statement.query_map([], |row| row.get::<_, String>(1))?;

        let mut found = false;

        for existing_column in columns {
            if existing_column? == column {
                found = true;
                break;
            }
        }

        if !found {
            let alter_table = format!(
                "ALTER TABLE {table} \
ADD COLUMN {column} INTEGER NOT NULL DEFAULT 0 \
CHECK({column} IN (0, 1));"
            );

            self.connection.execute(&alter_table, [])?;
        }

        Ok(())
    }

    pub fn backup_database<P>(&self, backup_directory: P) -> Result<PathBuf>
    where
        P: AsRef<Path>,
    {
        let backup_directory = backup_directory.as_ref();
        fs::create_dir_all(backup_directory)?;

        let timestamp: String = self.connection.query_row(
            "
            SELECT strftime(
                '%Y%m%d-%H%M%S',
                'now',
                'localtime'
        );
        ",
            [],
            |row| row.get(0),
        )?;

        let base_name = format!("StorageManager-backup-{timestamp}");
        let mut backup_path = backup_directory.join(format!("{base_name}.db"));
        let mut suffix = 1_u32;

        while backup_path.exists() {
            backup_path = backup_directory.join(format!("{base_name}-{suffix:02}.db"));
            suffix += 1;
        }

        let backup_filename = backup_path.to_string_lossy().into_owned();

        self.connection
            .execute("VACUUM INTO ?1;", params![backup_filename])?;

        let backup_connection = Connection::open(&backup_path)?;
        let integrity_result: String =
            backup_connection.query_row("PRAGMA integrity_check;", [], |row| row.get(0))?;

        drop(backup_connection);

        if !integrity_result.trim().eq_ignore_ascii_case("ok") {
            let _ = fs::remove_file(&backup_path);

            return Err(anyhow!(
                "The backup failed its integrity check: {}",
                integrity_result
            ));
        }

        Ok(backup_path)
    }

    pub fn restore_database<P, Q>(
        &mut self,
        backup_path: P,
        safety_backup_directory: Q,
    ) -> Result<PathBuf>
    where
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        let backup_path = backup_path.as_ref();

        if !backup_path.exists() {
            return Err(anyhow!(
                "Backup file does not exist: {}",
                backup_path.display()
            ));
        }

        if !backup_path.is_file() {
            return Err(anyhow!(
                "Backup path is not a file: {}",
                backup_path.display()
            ));
        }

        let source_connection = Connection::open(backup_path)?;
        let source_integrity: String =
            source_connection.query_row("PRAGMA integrity_check;", [], |row| row.get(0))?;

        if !source_integrity.trim().eq_ignore_ascii_case("ok") {
            return Err(anyhow!(
                "The selected backup failed its integrity check: {}",
                source_integrity
            ));
        }

        for required_table in ["units", "tenants", "rentals", "payments"] {
            let table_exists: i64 = source_connection.query_row(
                "
                SELECT COUNT(*)
            FROM sqlite_master
            WHERE
            type = 'table'
            AND name = ?1;
            ",
                params![required_table],
                |row| row.get(0),
            )?;

            if table_exists == 0 {
                return Err(anyhow!(
                    "The selected file is not a valid Storage Manager backup. Required table '{}' is missing.",
                    required_table
                ));
            }
        }

        let safety_backup_path = self.backup_database(safety_backup_directory)?;

        {
            let backup = Backup::new(&source_connection, &mut self.connection)?;
            backup.run_to_completion(5, Duration::from_millis(50), None)?;
        }

        self.connection.execute_batch("PRAGMA foreign_keys = ON;")?;

        self.initialize()?;
        self.ensure_units_archived_column()?;
        self.ensure_tenants_archived_column()?;
        self.ensure_units_needs_cleaned_column()?;
        self.ensure_units_lock_columns()?;
        self.ensure_units_layout_columns()?;
        self.reconcile_unit_occupancy()?;

        let restored_integrity: String =
            self.connection
                .query_row("PRAGMA integrity_check;", [], |row| row.get(0))?;

        if !restored_integrity.trim().eq_ignore_ascii_case("ok") {
            return Err(anyhow!(
                "The restored database failed its integrity check: {}. The pre-restore safety backup is located at {}.",
                restored_integrity,
                safety_backup_path.display()
            ));
        }

        Ok(safety_backup_path)
    }

    fn ensure_units_needs_cleaned_column(&self) -> Result<()> {
        let has_column = {
            let mut statement = self.connection.prepare("PRAGMA table_info(units);")?;

            let columns = statement.query_map([], |row| row.get::<_, String>(1))?;

            let mut found = false;

            for column in columns {
                if column? == "needs_cleaned" {
                    found = true;
                    break;
                }
            }

            found
        };

        if !has_column {
            self.connection.execute(
                "
                ALTER TABLE units
                ADD COLUMN needs_cleaned INTEGER NOT NULL
                DEFAULT 0
                CHECK(needs_cleaned IN (0, 1));
            ",
                [],
            )?;
        }

        Ok(())
    }

    fn ensure_units_lock_columns(&self) -> Result<()> {
        self.ensure_boolean_column("units", "yellow_lock")?;
        self.ensure_boolean_column("units", "red_lock")?;

        Ok(())
    }

    fn ensure_units_layout_columns(&self) -> Result<()> {
        let pragma = "PRAGMA table_info(units);";
        let mut statement = self.connection.prepare(pragma)?;
        let columns = statement.query_map([], |row| row.get::<_, String>(1))?;

        let mut has_layout_section = false;
        let mut has_display_order = false;

        for column in columns {
            match column?.as_str() {
                "layout_section" => has_layout_section = true,
                "display_order" => has_display_order = true,
                _ => {}
            }
        }

        drop(statement);

        if !has_layout_section {
            self.connection.execute(
                "ALTER TABLE units ADD COLUMN layout_section TEXT NOT NULL DEFAULT '';",
                [],
            )?;
        }

        if !has_display_order {
            self.connection.execute(
                "ALTER TABLE units ADD COLUMN display_order INTEGER NOT NULL DEFAULT 0 CHECK(display_order >= 0);",
                [],
            )?;
        }

        self.connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_units_layout ON units(building, layout_section, display_order);",
            [],
        )?;

        Ok(())
    }

    pub fn set_unit_needs_cleaned(&self, id: i64, needs_cleaned: bool) -> Result<bool> {
        if needs_cleaned {
            let occupied: i64 = self.connection.query_row(
                "
                SELECT occupied
                FROM units
                WHERE id = ?1;
                ",
                params![id],
                |row| row.get(0),
            )?;

            if occupied != 0 {
                return Err(anyhow!("An occupied unit cannot be marked Needs Cleaned."));
            }
        }

        let affected_rows = self.connection.execute(
            "
            UPDATE units
            SET
            needs_cleaned = ?1,
            updated_at = CURRENT_TIMESTAMP
            WHERE
            id = ?2
            AND occupied = 0;
            ",
            params![needs_cleaned as i64, id,],
        )?;

        Ok(affected_rows == 1)
    }
}

fn map_unit(row: &rusqlite::Row<'_>) -> rusqlite::Result<Unit> {
    Ok(Unit {
        id: row.get(0)?,
        unit_number: row.get(1)?,
        building: row.get(2)?,
        width: row.get(3)?,
        length: row.get(4)?,
        monthly_rent_cents: row.get(5)?,
        occupied: row.get::<_, i64>(6)? != 0,
        needs_cleaned: row.get::<_, i64>(7)? != 0,
        yellow_lock: row.get::<_, i64>(8)? != 0,
        red_lock: row.get::<_, i64>(9)? != 0,
        layout_section: row.get(10)?,
        display_order: row.get(11)?,
        notes: row.get(12)?,
    })
}

fn map_tenant(row: &rusqlite::Row<'_>) -> rusqlite::Result<Tenant> {
    Ok(Tenant {
        id: row.get(0)?,
        first_name: row.get(1)?,
        last_name: row.get(2)?,
        phone: row.get(3)?,
        email: row.get(4)?,
        active: row.get::<_, i64>(5)? != 0,
        notes: row.get(6)?,
    })
}
