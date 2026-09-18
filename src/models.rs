/// A storage unit already saved in the database.
#[derive(Debug, Clone)]
pub struct Unit {
    pub id: i64,
    pub unit_number: String,
    pub building: String,
    pub width: Option<f64>,
    pub length: Option<f64>,
    pub monthly_rent_cents: i64,
    pub occupied: bool,
    pub needs_cleaned: bool,
    pub yellow_lock: bool,
    pub red_lock: bool,
    pub layout_section: String,
    pub display_order: i64,
    pub notes: String,
}

/// Information needed to create a storage unit.
///
/// There is no ID because SQLite assigns it automatically.
#[derive(Debug, Clone, Default)]
pub struct NewUnit {
    pub unit_number: String,
    pub building: String,
    pub width: Option<f64>,
    pub length: Option<f64>,
    pub monthly_rent_cents: i64,
    pub yellow_lock: bool,
    pub red_lock: bool,
    pub layout_section: String,
    pub display_order: i64,
    pub notes: String,
}

#[derive(Debug, Clone)]
pub struct Tenant {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    pub phone: String,
    pub email: String,
    pub active: bool,
    pub notes: String,
}

impl Tenant {
    pub fn display_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name,)
    }
}

#[derive(Debug, Clone)]
pub struct NewTenant {
    pub first_name: String,
    pub last_name: String,
    pub phone: String,
    pub email: String,
    pub active: bool,
    pub notes: String,
}

impl Default for NewTenant {
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

#[derive(Debug, Clone)]
pub struct Rental {
    pub id: i64,
    pub tenant_name: String,
    pub unit_number: String,
    pub start_date: String,
    pub monthly_rent_cents: i64,
}

#[derive(Debug, Clone, Default)]
pub struct NewRental {
    pub tenant_id: i64,
    pub unit_id: i64,
    pub start_date: String,
    pub monthly_rent_cents: i64,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaymentMethod {
    #[default]
    Cash,
    Check,
    Card,
    BankTransfer,
    Other,
}

#[derive(Debug, Clone)]
pub struct Padlock {
    pub id: i64,
    pub serial_number: String,
    pub combination: String,
    pub unit_id: Option<i64>,
    pub unit_number: Option<String>,
    pub notes: String,
}

#[derive(Debug, Clone)]
pub struct NewPadlock {
    pub serial_number: String,
    pub combination: String,
    pub unit_id: Option<i64>,
    pub notes: String,
}

impl PaymentMethod {
    pub const ALL: [PaymentMethod; 5] = [
        PaymentMethod::Cash,
        PaymentMethod::Check,
        PaymentMethod::Card,
        PaymentMethod::BankTransfer,
        PaymentMethod::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PaymentMethod::Cash => "Cash",
            PaymentMethod::Check => "Check",
            PaymentMethod::Card => "Card",
            PaymentMethod::BankTransfer => "Bank Transfer",
            PaymentMethod::Other => "Other",
        }
    }

    pub fn from_database(value: &str) -> Self {
        match value {
            "Cash" => PaymentMethod::Cash,
            "Check" => PaymentMethod::Check,
            "Card" => PaymentMethod::Card,
            "Bank Transfer" => PaymentMethod::BankTransfer,
            _ => PaymentMethod::Other,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Payment {
    pub tenant_name: String,
    pub unit_number: String,
    pub payment_date: String,
    pub amount_cents: i64,
    pub payment_method: PaymentMethod,
    pub reference: String,
}

#[derive(Debug, Clone, Default)]
pub struct NewPayment {
    pub rental_id: i64,
    pub payment_date: String,
    pub amount_cents: i64,
    pub payment_method: PaymentMethod,
    pub reference: String,
    pub notes: String,
}

#[derive(Debug, Clone, Default)]
pub struct ReportSummary {
    pub total_units: i64,
    pub occupied_units: i64,
    pub vacant_units: i64,
    pub needs_cleaned_units: i64,
    pub active_tenants: i64,
    pub active_rentals: i64,
    pub monthly_rent_cents: i64,
    pub total_payments_cents: i64,
}

impl ReportSummary {
    pub fn occupancy_percent(&self) -> f64 {
        if self.total_units == 0 {
            return 0.0;
        }

        self.occupied_units as f64 / self.total_units as f64 * 100.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnitSize {
    #[default]
    FiveByFive,

    TenByTen,

    TenByFifteen,

    TenByTwenty,

    TenByThirty,
}

impl UnitSize {
    pub const ALL: [UnitSize; 5] = [
        UnitSize::FiveByFive,
        UnitSize::TenByTen,
        UnitSize::TenByFifteen,
        UnitSize::TenByTwenty,
        UnitSize::TenByThirty,
    ];

    pub fn label(self) -> &'static str {
        match self {
            UnitSize::FiveByFive => "5ft × 5ft",
            UnitSize::TenByTen => "10ft × 10ft",
            UnitSize::TenByFifteen => "10ft × 15ft",
            UnitSize::TenByTwenty => "10ft × 20ft",
            UnitSize::TenByThirty => "10ft × 30ft",
        }
    }

    pub fn dimensions(self) -> (f64, f64) {
        match self {
            UnitSize::FiveByFive => (5.0, 5.0),
            UnitSize::TenByTen => (10.0, 10.0),
            UnitSize::TenByFifteen => (10.0, 15.0),
            UnitSize::TenByTwenty => (10.0, 20.0),
            UnitSize::TenByThirty => (10.0, 30.0),
        }
    }

    /// Converts stored dimensions back into a standard size.
    ///
    /// This will be useful when we build Edit Unit.
    pub fn from_dimensions(width: Option<f64>, length: Option<f64>) -> Option<Self> {
        match (width, length) {
            (Some(5.0), Some(5.0)) => Some(UnitSize::FiveByFive),

            (Some(10.0), Some(10.0)) => Some(UnitSize::TenByTen),

            (Some(10.0), Some(15.0)) => Some(UnitSize::TenByFifteen),

            (Some(10.0), Some(20.0)) => Some(UnitSize::TenByTwenty),

            (Some(10.0), Some(30.0)) => Some(UnitSize::TenByThirty),

            _ => None,
        }
    }
}
