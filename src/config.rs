// SPDX-License-Identifier: GPL-3.0-only

use cosmic::cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry};
use serde::{Deserialize, Serialize};
pub use weathervane::{MeasurementSystem, PressureUnit, SavedLocation, TemperatureUnit};

/// Tab options for the popup interface.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PopupTab {
    #[default]
    Current,
    Alerts,
    Hourly,
    Forecast,
    Graph,
    Settings,
}

#[derive(Debug, Clone, CosmicConfigEntry, PartialEq, Serialize, Deserialize)]
#[version = 1]
pub struct Config {
    pub latitude: f64,
    pub longitude: f64,
    pub location_name: String,
    pub temperature_unit: TemperatureUnit,
    pub measurement_system: MeasurementSystem,
    /// Pressure display unit (hPa, inHg, or PSI).
    #[serde(default)]
    pub pressure_unit: PressureUnit,
    pub refresh_interval_minutes: u64,
    pub use_auto_location: bool,
    /// Stores the manual location when auto-detect is enabled, so it can be restored.
    pub manual_latitude: Option<f64>,
    pub manual_longitude: Option<f64>,
    pub manual_location_name: Option<String>,
    pub last_updated: Option<i64>,
    /// Last selected tab, restored on popup open.
    #[serde(default)]
    pub default_tab: PopupTab,
    /// Enable weather alerts (US via NWS, EU via MeteoAlarm).
    #[serde(default = "default_true")]
    pub alerts_enabled: bool,
    /// Automatically select units based on detected location.
    #[serde(default = "default_true")]
    pub auto_units: bool,
    /// Show AQI in the panel display.
    #[serde(default = "default_true")]
    pub show_aqi_in_panel: bool,
    /// Show weather icon in the panel display.
    #[serde(default = "default_true")]
    pub show_icon_in_panel: bool,
    /// Show pressure in the panel display.
    #[serde(default)]
    pub show_pressure_in_panel: bool,
    /// Show dew point in the panel display.
    #[serde(default)]
    pub show_dew_point_in_panel: bool,
    /// Show sunrise/sunset times in the panel display.
    #[serde(default)]
    pub show_sunrise_sunset_in_panel: bool,
    /// Show the meteogram Graph tab in the popup.
    #[serde(default = "default_show_meteogram")]
    pub show_meteogram: bool,
    /// Show the Pride Month rainbow accent (popup stripe + panel accent) during June.
    #[serde(default = "default_pride_accent")]
    pub pride_accent: bool,
    /// Bookmarked locations for quick switching.
    #[serde(default)]
    pub saved_locations: Vec<SavedLocation>,
    /// User-supplied aqicn.org token. When set, non-European locations
    /// use aqicn for the headline AQI instead of Open-Meteo.
    #[serde(default)]
    pub aqicn_token: Option<String>,
    /// Country of the current location, carried from detect/select results.
    /// Used by unit-selection and AQI-source logic in later plans.
    #[serde(default)]
    pub country: Option<String>,
}

/// Shared serde default for always-on booleans (alerts, auto-units, AQI panel,
/// icon panel). Consolidates four identical `default_*` fns (DRY-07).
fn default_true() -> bool {
    true
}

fn default_show_meteogram() -> bool {
    true // D-13: ON for new AND migrating users
}

fn default_pride_accent() -> bool {
    true // ON for new AND migrating users (additive serde-defaulted field, version unchanged)
}

// DRY-07 drift-risk: the literal bool defaults below (e.g. `alerts_enabled: true`)
// are a SECOND source of truth, parallel to the `#[serde(default = "...")]`
// attributes on the struct fields. serde defaults apply when deserializing an
// older on-disk config missing a field; this `impl Default` applies when building
// a fresh Config. They must stay in sync — editing one without the other lets the
// two silently diverge (a field could default true on migration but false on first run).
impl Default for Config {
    fn default() -> Self {
        Self {
            latitude: 40.7128,
            longitude: -74.0060,
            location_name: "New York, NY, United States".to_string(),
            temperature_unit: TemperatureUnit::default(),
            measurement_system: MeasurementSystem::default(),
            pressure_unit: PressureUnit::default(),
            refresh_interval_minutes: 15,
            use_auto_location: true,
            manual_latitude: None,
            manual_longitude: None,
            manual_location_name: None,
            last_updated: None,
            default_tab: PopupTab::default(),
            alerts_enabled: true,
            auto_units: true,
            show_aqi_in_panel: true,
            show_icon_in_panel: true,
            show_pressure_in_panel: false,
            show_dew_point_in_panel: false,
            show_sunrise_sunset_in_panel: false,
            show_meteogram: true,
            pride_accent: true,
            saved_locations: Vec::new(),
            aqicn_token: None,
            country: None,
        }
    }
}
