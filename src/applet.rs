// SPDX-License-Identifier: GPL-3.0-only

use cosmic::app::{Core, Task};
use cosmic::cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry};
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::window::Id;
use cosmic::iced::{Limits, Subscription};
use cosmic::widget::{canvas, segmented_button};
use cosmic::{Action, Application, Element};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;

use crate::config::{Config, MeasurementSystem, PopupTab, PressureUnit, TemperatureUnit};
use crate::weather::{
    detect_location, fetch_air_quality, fetch_alerts, fetch_pollen, fetch_weather, search_city,
    uses_imperial_units, AirQualityData, Alert, AlertSeverity, AqiStandard, DetectedLocation,
    LocationResult, PollenData, WeatherData,
};

mod view;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// System-wide time format preference from COSMIC time applet.
#[derive(Debug, Clone, Default, PartialEq, Eq, CosmicConfigEntry, Deserialize, Serialize)]
#[version = 1]
pub struct TimeAppletConfig {
    #[serde(default)]
    pub military_time: bool,
}

pub struct Tempest {
    core: Core,
    /// The popup id.
    popup: Option<Id>,
    /// Weather data.
    weather_data: Option<WeatherData>,
    /// Air quality data.
    air_quality: Option<AirQualityData>,
    /// Active weather alerts.
    alerts: Vec<Alert>,
    /// IDs of alerts already shown as notifications (prevents duplicates).
    seen_alert_ids: HashSet<String>,
    /// Configuration
    config: Config,
    /// Config handler for persistence
    config_handler: Option<cosmic::cosmic_config::Config>,
    /// Input field states
    city_input: String,
    refresh_input: String,
    aqicn_token_input: String,
    /// Search results
    search_results: Vec<LocationResult>,
    /// Display label for panel button
    display_label: String,
    /// Current weather condition for icon display
    current_condition: weathervane::WeatherCondition,
    /// Current AQI for panel display
    current_aqi: Option<(i32, AqiStandard)>,
    /// Loading state
    is_loading: bool,
    /// Error state
    error_message: Option<String>,
    /// A refresh failed while we still hold valid weather. Drives the stale
    /// marker on the "Updated at" header instead of the full error view, so a
    /// brief network blip doesn't blow away the last good reading.
    refresh_failed: bool,
    /// Active tab in the popup
    active_tab: PopupTab,
    /// Segmented control model for tab selection
    tab_model: segmented_button::SingleSelectModel,
    /// Segmented control model for temperature unit selection
    temperature_model: segmented_button::SingleSelectModel,
    /// Segmented control model for measurement system selection
    measurement_model: segmented_button::SingleSelectModel,
    /// Segmented control model for pressure unit selection
    pressure_model: segmented_button::SingleSelectModel,
    /// Cached formatted timestamp for display (avoids recomputing on every render)
    last_updated_display: Option<String>,
    /// 24-hour time format when true, 12-hour with AM/PM when false.
    military_time: bool,
    /// Whether the pollutants sub-view is currently displayed.
    showing_pollutants: bool,
    /// Whether the saved locations sub-view is currently displayed.
    showing_locations: bool,
    /// Pollen readings, when covered. Outer Option distinguishes
    /// "not yet fetched" from inner Option's "fetched but uncovered"
    /// (Ok(None) from the API for non-CAMS coordinates).
    pollen: Option<Option<PollenData>>,
    /// Whether the pollen drill-down sub-view is currently displayed.
    showing_pollen: bool,
    /// Set when a save is attempted at the 8-slot cap; renders an
    /// inline caption in the search section instead of silently clearing the
    /// search. Cleared on the next successful save or search-input change.
    saved_locations_full: bool,
    /// Cached max popup height based on screen resolution.
    popup_max_height: f32,
    /// Consecutive fetch failures driving the backoff retry schedule.
    retry_count: u8,
    /// Monotonic request-generation counter. Bumped at every logical fetch
    /// start so superseded in-flight results can be discarded — a slow old
    /// response can't overwrite the data from a newer request.
    fetch_generation: u64,
    /// Shared tessellation cache for the meteogram canvas. Borrowed by
    /// `Meteogram` so `draw()` reuses geometry across renders; cleared only at the
    /// state transitions that change rendered pixels (weather replace, hourly tick,
    /// 12/24h format change, dark/light theme change).
    meteogram_cache: canvas::Cache,
}

/// Fallback popup max height (px) used until the async resolution query resolves,
/// and whenever cosmic-randr reports no usable display. Assumes ~1080p.
const POPUP_MAX_HEIGHT_FALLBACK: f32 = 650.0;

/// Queries cosmic-randr for the primary display resolution without blocking.
/// Returns (width, height) or None if unavailable. Awaits the subprocess future
/// directly so it can run off the startup critical path rather than blocking
/// the applet's first paint.
async fn get_screen_resolution_async() -> Option<(u32, u32)> {
    let list = cosmic_randr_shell::list().await.ok()?;

    for (_key, output) in &list.outputs {
        if output.enabled {
            if let Some(mode_key) = output.current {
                if let Some(mode) = list.modes.get(mode_key) {
                    return Some(mode.size);
                }
            }
        }
    }
    None
}

/// Returns the tab as an Option, with Settings/Alerts mapped to None
/// since they're not part of the segmented control.
fn tab_for_segmented_control(tab: PopupTab) -> Option<PopupTab> {
    match tab {
        PopupTab::Settings | PopupTab::Alerts => None,
        other => Some(other),
    }
}

/// Builds the segmented control model for tab selection.
/// Pass `None` to build with no active selection (for Settings/Alerts tabs).
/// The Graph segment is appended as the 4th tab only when `show_meteogram` is
/// enabled — disabling the meteogram removes the Graph segment from the bar.
fn build_tab_model(
    active: Option<PopupTab>,
    show_meteogram: bool,
) -> segmented_button::SingleSelectModel {
    let mut model = segmented_button::SingleSelectModel::default();

    let mut tabs = vec![
        (PopupTab::Current, crate::fl!("tab-current")),
        (PopupTab::Hourly, crate::fl!("tab-hourly")),
        (PopupTab::Forecast, crate::fl!("tab-forecast")),
    ];
    if show_meteogram {
        tabs.push((PopupTab::Graph, crate::fl!("tab-graph")));
    }

    for (tab, label) in tabs {
        let id = model.insert().text(label).data(tab).id();
        if active == Some(tab) {
            model.activate(id);
        }
    }

    model
}

/// Builds the segmented control model for temperature unit selection.
fn build_temperature_model(active: TemperatureUnit) -> segmented_button::SingleSelectModel {
    let mut model = segmented_button::SingleSelectModel::default();

    let units = [
        (TemperatureUnit::Celsius, TemperatureUnit::Celsius.symbol()),
        (
            TemperatureUnit::Fahrenheit,
            TemperatureUnit::Fahrenheit.symbol(),
        ),
    ];

    for (unit, label) in units {
        let id = model.insert().text(label).data(unit).id();
        if unit == active {
            model.activate(id);
        }
    }

    model
}

/// Builds the segmented control model for measurement system selection.
fn build_measurement_model(active: MeasurementSystem) -> segmented_button::SingleSelectModel {
    let mut model = segmented_button::SingleSelectModel::default();

    let systems = [
        (MeasurementSystem::Metric, crate::fl!("unit-metric")),
        (MeasurementSystem::Imperial, crate::fl!("unit-imperial")),
    ];

    for (system, label) in systems {
        let id = model.insert().text(label).data(system).id();
        if system == active {
            model.activate(id);
        }
    }

    model
}

/// Builds the segmented control model for pressure unit selection.
fn build_pressure_model(active: PressureUnit) -> segmented_button::SingleSelectModel {
    let mut model = segmented_button::SingleSelectModel::default();

    let units = [
        (PressureUnit::Hpa, PressureUnit::Hpa.symbol()),
        (PressureUnit::InHg, PressureUnit::InHg.symbol()),
        (PressureUnit::Psi, PressureUnit::Psi.symbol()),
    ];

    for (unit, label) in units {
        let id = model.insert().text(label).data(unit).id();
        if unit == active {
            model.activate(id);
        }
    }

    model
}

impl Default for Tempest {
    fn default() -> Self {
        let config = Config::default();
        let active_tab = PopupTab::default();
        Self {
            core: Default::default(),
            popup: None,
            weather_data: None,
            air_quality: None,
            alerts: Vec::new(),
            seen_alert_ids: HashSet::new(),
            city_input: String::new(),
            refresh_input: config.refresh_interval_minutes.to_string(),
            aqicn_token_input: String::new(),
            search_results: Vec::new(),
            display_label: "...".to_string(),
            current_condition: weathervane::WeatherCondition::Unknown,
            current_aqi: None,
            is_loading: true,
            error_message: None,
            refresh_failed: false,
            active_tab,
            tab_model: build_tab_model(
                tab_for_segmented_control(active_tab),
                config.show_meteogram,
            ),
            temperature_model: build_temperature_model(config.temperature_unit),
            measurement_model: build_measurement_model(config.measurement_system),
            pressure_model: build_pressure_model(config.pressure_unit),
            last_updated_display: None,
            military_time: false,
            showing_pollutants: false,
            showing_locations: false,
            pollen: None,
            showing_pollen: false,
            saved_locations_full: false,
            popup_max_height: POPUP_MAX_HEIGHT_FALLBACK,
            retry_count: 0,
            fetch_generation: 0,
            meteogram_cache: canvas::Cache::new(),
            config,
            config_handler: None,
        }
    }
}

/// Message variants for application communication.
#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    /// Async result of the cosmic-randr resolution query (run off the startup
    /// critical path). `None` => no usable display; the handler keeps the 650px
    /// fallback.
    ScreenResolution(Option<(u32, u32)>),
    RefreshWeather,
    WeatherUpdated(u64, Result<WeatherData, String>),
    AirQualityUpdated(u64, Result<AirQualityData, String>),
    AlertsUpdated(u64, Result<Vec<Alert>, String>),
    Tick,
    ToggleAlertsEnabled,
    ToggleAutoUnits,
    ToggleShowAqiInPanel,
    ToggleShowIconInPanel,
    ToggleShowPressureInPanel,
    ToggleShowDewPointInPanel,
    ToggleShowSunriseSunsetInPanel,
    ToggleShowMeteogram,
    TogglePrideAccent,
    UpdateCityInput(String),
    SearchCity,
    CitySearchResult(Result<Vec<LocationResult>, String>),
    SelectLocation(usize),
    UpdateRefreshInterval(String),
    UpdateAqicnToken(String),
    /// Enter/submit commit for the refresh-interval field. Edits are buffered
    /// locally and only written on commit, avoiding a config write per keystroke.
    /// Reads from `refresh_input`; the `on_submit` String payload is ignored.
    CommitRefreshInterval,
    /// Enter/submit commit for the AQI-token field. Edits are buffered locally
    /// and only written on commit, avoiding a config write per keystroke.
    /// Reads from `aqicn_token_input`; the `on_submit` String payload is ignored.
    CommitAqicnToken,
    DetectLocation,
    LocationDetected(Result<DetectedLocation, String>),
    ToggleAutoLocation,
    SelectTab(PopupTab),
    TabActivated(segmented_button::Entity),
    TemperatureUnitActivated(segmented_button::Entity),
    MeasurementActivated(segmented_button::Entity),
    PressureUnitActivated(segmented_button::Entity),
    SystemTimeConfig(TimeAppletConfig),
    ShowPollutants,
    HidePollutants,
    PollenUpdated(u64, Result<Option<PollenData>, String>),
    ShowPollen,
    HidePollen,
    SaveLocation(usize),
    RemoveSavedLocation(usize),
    ShowLocations,
    HideLocations,
    SwitchLocation(usize),
    OpenKofi,
    OpenSourceCode,
    OpenWorkItems,
    RetryFetch,
    NetworkChanged(crate::network::NetworkEvent),
    SystemResumed,
}

impl Application for Tempest {
    type Executor = cosmic::executor::Default;

    type Flags = ();

    type Message = Message;

    const APP_ID: &'static str = "com.vintagetechie.CosmicExtAppletTempest";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let config_handler = cosmic::cosmic_config::Config::new(Self::APP_ID, Config::VERSION).ok();
        let mut config = config_handler
            .as_ref()
            .and_then(|h| Config::get_entry(h).ok())
            .unwrap_or_default();

        // Seed saved locations from the active location on first run
        if config.saved_locations.is_empty() && !config.location_name.is_empty() {
            config.saved_locations.push(crate::config::SavedLocation {
                name: config.location_name.clone(),
                latitude: config.latitude,
                longitude: config.longitude,
            });
            if let Some(ref handler) = config_handler {
                if let Err(e) = config.write_entry(handler) {
                    tracing::error!("Failed to save migrated config: {}", e);
                }
            }
        }

        let refresh_input = config.refresh_interval_minutes.to_string();
        // A persisted `default_tab == Graph` while the meteogram is disabled
        // (e.g. a hand-edited config) must not open the popup to a missing view —
        // fall back to Current. The Graph segment is also omitted from the tab bar
        // below, so there is no clickable path back to the absent view.
        let active_tab = if config.default_tab == PopupTab::Graph && !config.show_meteogram {
            PopupTab::Current
        } else {
            config.default_tab
        };
        let tab_model =
            build_tab_model(tab_for_segmented_control(active_tab), config.show_meteogram);
        let temperature_model = build_temperature_model(config.temperature_unit);
        let measurement_model = build_measurement_model(config.measurement_system);
        let pressure_model = build_pressure_model(config.pressure_unit);

        // Read system time format preference for immediate correct display
        let military_time = cosmic::cosmic_config::Config::new(
            "com.system76.CosmicAppletTime",
            TimeAppletConfig::VERSION,
        )
        .ok()
        .and_then(|h| TimeAppletConfig::get_entry(&h).ok())
        .map(|c| c.military_time)
        .unwrap_or(false);

        let mut app = Tempest {
            core,
            config: config.clone(),
            config_handler,
            city_input: String::new(),
            refresh_input,
            aqicn_token_input: config.aqicn_token.clone().unwrap_or_default(),
            search_results: Vec::new(),
            display_label: "...".to_string(),
            active_tab,
            tab_model,
            temperature_model,
            measurement_model,
            pressure_model,
            military_time,
            // Remaining fields built explicitly (no ..Default::default(), which
            // would re-run the now-deleted blocking resolution query) — values
            // match the Default impl.
            popup: None,
            weather_data: None,
            air_quality: None,
            alerts: Vec::new(),
            seen_alert_ids: HashSet::new(),
            current_condition: weathervane::WeatherCondition::Unknown,
            current_aqi: None,
            is_loading: true,
            error_message: None,
            refresh_failed: false,
            last_updated_display: None,
            showing_pollutants: false,
            showing_locations: false,
            pollen: None,
            showing_pollen: false,
            saved_locations_full: false,
            popup_max_height: POPUP_MAX_HEIGHT_FALLBACK,
            retry_count: 0,
            fetch_generation: 0,
            meteogram_cache: canvas::Cache::new(),
        };

        // Seed the "Updated at HH:MM" header from the persisted
        // timestamp so a freshly-started applet shows the last known refresh
        // time instead of a blank header until the first fetch completes. Same
        // reconstruction as `handle_system_time_config`; runs after `app` exists
        // because `format_time_of_day` is a method on `self`.
        if let Some(timestamp) = app.config.last_updated {
            if let Some(dt) = chrono::DateTime::from_timestamp(timestamp, 0) {
                let local = dt.with_timezone(&chrono::Local);
                app.last_updated_display = Some(app.format_time_of_day(local));
            }
        }

        // Start with auto-location if enabled, otherwise fetch weather
        let task = if config.use_auto_location {
            Self::detect_location_task()
        } else {
            Task::perform(async { Message::RefreshWeather }, Action::App)
        };

        // Fire the resolution query off the startup critical path:
        // the panel button renders immediately on the 650px fallback; the async
        // result refines popup_max_height when it arrives.
        (app, Task::batch([task, Self::screen_resolution_task()]))
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let interval_minutes = self.config.refresh_interval_minutes;

        // Periodic weather refresh
        let tick = Subscription::run_with(
            (std::any::TypeId::of::<Self>(), interval_minutes),
            |&(_, mins)| {
                async_stream::stream! {
                    let interval = Duration::from_secs(mins * 60);
                    loop {
                        tokio::time::sleep(interval).await;
                        yield Message::Tick;
                    }
                }
            },
        );

        // Watch system time config for 12/24 hour format changes
        let time_config = self
            .core
            .watch_config::<TimeAppletConfig>("com.system76.CosmicAppletTime")
            .map(|update| Message::SystemTimeConfig(update.config));

        // Monitor NetworkManager for connectivity changes
        let network = crate::network::network_subscription().map(Message::NetworkChanged);

        // Monitor systemd-logind for resume from suspend
        let sleep = crate::sleep::sleep_subscription().map(|_| Message::SystemResumed);

        Subscription::batch([tick, time_config, network, sleep])
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    /// Dark/light theme seam: the meteogram's `is_dark` branch
    /// selects different series and chrome colors, so a theme-mode flip must
    /// invalidate the cached chart for an instant repaint.
    fn system_theme_mode_update(
        &mut self,
        _keys: &[&'static str],
        _new_theme: &cosmic::cosmic_theme::ThemeMode,
    ) -> Task<Self::Message> {
        self.meteogram_cache.clear();
        Task::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        self.view_panel()
    }

    fn view_window(&self, id: Id) -> Element<'_, Self::Message> {
        self.view_popup(id)
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::TogglePopup => return self.handle_toggle_popup(),
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    // Dismiss/outside-click close route — commit pending
                    // text-field edits and persist the last-used tab on close
                    // (rather than on every keystroke). Second of two close
                    // routes; see handle_toggle_popup.
                    self.commit_pending_edits();
                    self.popup = None;
                    // Reset sub-view overlays so a reopen lands on the
                    // tab, not a stale overlay.
                    self.showing_pollutants = false;
                    self.showing_pollen = false;
                    self.showing_locations = false;
                }
            }
            Message::ScreenResolution(res) => {
                // Latest-wins: a height value needs no generation guard.
                // Mirrors the arithmetic of the former calculate_popup_max_height().
                self.popup_max_height = res
                    .map(|(_, h)| (h as f32 * 0.75).clamp(400.0, 1000.0))
                    .unwrap_or(POPUP_MAX_HEIGHT_FALLBACK);
            }
            Message::RefreshWeather => return self.handle_refresh_weather(),
            Message::WeatherUpdated(gen, result) => {
                return self.handle_weather_updated(gen, result);
            }
            Message::AirQualityUpdated(gen, result) => {
                // Drop superseded results before touching any state, so a slow
                // old-coords response can't overwrite the current location's AQI.
                if !is_current_generation(self.fetch_generation, gen) {
                    return Task::none();
                }
                match result {
                    Ok(data) => {
                        self.current_aqi = Some((data.aqi, data.standard()));
                        self.air_quality = Some(data);
                    }
                    Err(e) => {
                        // Suppress the transient error and keep the last good
                        // AQI/air-quality in place (same spirit as pollen's
                        // suppress-transients tri-state), so the AQI row stays
                        // stable across wifi blips instead of blanking.
                        tracing::warn!("Failed to fetch air quality: {}", e);
                    }
                }
            }
            Message::AlertsUpdated(gen, result) => {
                // Drop superseded results before seen_alert_ids insertion,
                // so a stale alert batch can't mark IDs seen and suppress a real one.
                if !is_current_generation(self.fetch_generation, gen) {
                    return Task::none();
                }
                match result {
                    Ok(new_alerts) => {
                        for alert in &new_alerts {
                            if !self.seen_alert_ids.contains(&alert.id) {
                                self.send_alert_notification(alert);
                                self.seen_alert_ids.insert(alert.id.clone());
                            }
                        }
                        self.alerts = new_alerts;
                        // Prune the seen-set to the IDs in this winning batch so
                        // it stays bounded to currently-active alerts. An alert
                        // that clears then genuinely re-issues re-notifies. Runs
                        // only here — inside the Ok branch, past the generation
                        // guard — so a stale or superseded batch can never
                        // reshape the seen-set.
                        let batch_ids: HashSet<_> =
                            self.alerts.iter().map(|a| a.id.clone()).collect();
                        self.seen_alert_ids.retain(|id| batch_ids.contains(id));
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch alerts: {}", e);
                    }
                }
            }
            Message::Tick => {
                self.retry_count = 0;
                // Hourly wall-clock advance: refresh now-marker, night
                // shading, and current-hour index at hourly granularity.
                self.meteogram_cache.clear();
                return Self::refresh_task();
            }
            Message::ToggleAlertsEnabled => {
                self.config.alerts_enabled = !self.config.alerts_enabled;
                if !self.config.alerts_enabled {
                    self.alerts.clear();
                }
                self.save_config();
                return Self::refresh_task();
            }
            Message::ToggleAutoUnits => {
                self.config.auto_units = !self.config.auto_units;
                if self.config.auto_units {
                    // Apply units for the persisted structured country (populated on
                    // detect/select). A missing country simply skips auto-units.
                    if let Some(country) = self.config.country.clone() {
                        self.apply_units_for_country(&country);
                    }
                }
                self.save_config();
                return Self::refresh_task();
            }
            Message::ToggleShowAqiInPanel => {
                self.config.show_aqi_in_panel = !self.config.show_aqi_in_panel;
                self.save_config();
            }
            Message::ToggleShowIconInPanel => {
                self.config.show_icon_in_panel = !self.config.show_icon_in_panel;
                self.save_config();
            }
            Message::ToggleShowPressureInPanel => {
                self.config.show_pressure_in_panel = !self.config.show_pressure_in_panel;
                self.save_config();
            }
            Message::ToggleShowDewPointInPanel => {
                self.config.show_dew_point_in_panel = !self.config.show_dew_point_in_panel;
                self.save_config();
            }
            Message::ToggleShowSunriseSunsetInPanel => {
                self.config.show_sunrise_sunset_in_panel =
                    !self.config.show_sunrise_sunset_in_panel;
                self.save_config();
            }
            Message::ToggleShowMeteogram => {
                self.config.show_meteogram = !self.config.show_meteogram;
                // Disabling the meteogram removes the Graph segment, so a
                // Graph active/default tab would point at a missing view — reset
                // both to Current before rebuilding the tab bar.
                if !self.config.show_meteogram
                    && (self.active_tab == PopupTab::Graph
                        || self.config.default_tab == PopupTab::Graph)
                {
                    self.active_tab = PopupTab::Current;
                    self.config.default_tab = PopupTab::Current;
                }
                self.tab_model = build_tab_model(
                    tab_for_segmented_control(self.active_tab),
                    self.config.show_meteogram,
                );
                self.save_config();
            }
            Message::TogglePrideAccent => {
                self.config.pride_accent = !self.config.pride_accent;
                self.save_config();
            }
            Message::UpdateCityInput(value) => {
                self.city_input = value;
                // Clear the save-cap feedback once the user edits the
                // search again.
                self.saved_locations_full = false;
            }
            Message::SearchCity => {
                let city = self.city_input.clone();
                if !city.is_empty() {
                    return Task::perform(
                        async move { search_city(&city).await.map_err(|e| e.to_string()) },
                        |result| Action::App(Message::CitySearchResult(result)),
                    );
                }
            }
            Message::CitySearchResult(result) => match result {
                Ok(results) => {
                    self.search_results = results;
                }
                Err(e) => {
                    tracing::warn!("City search failed: {}", e);
                    self.search_results.clear();
                }
            },
            Message::SelectLocation(idx) => return self.handle_select_location(idx),
            // Local-edit-only. No parse, no config write, no save per
            // keystroke — committed on Enter (CommitRefreshInterval) and on popup
            // close (commit_pending_edits). This stops the "typing 120 passes
            // through a live 1-minute tick" subscription thrash.
            Message::UpdateRefreshInterval(value) => {
                self.refresh_input = value;
            }
            // Local-edit-only; committed on Enter / popup close.
            Message::UpdateAqicnToken(value) => {
                self.aqicn_token_input = value;
            }
            Message::CommitRefreshInterval => self.commit_refresh_interval(),
            Message::CommitAqicnToken => self.commit_aqicn_token(),
            Message::ToggleAutoLocation => return self.handle_toggle_auto_location(),
            Message::DetectLocation => return Self::detect_location_task(),
            Message::LocationDetected(result) => return self.handle_location_detected(result),
            // default_tab now persists once on popup close (via
            // commit_pending_edits), not per tab click. These handlers only
            // update transient state; observable reopen behavior is unchanged.
            Message::SelectTab(tab) => {
                self.select_tab(tab);
                self.tab_model =
                    build_tab_model(tab_for_segmented_control(tab), self.config.show_meteogram);
            }
            Message::TabActivated(entity) => {
                self.tab_model.activate(entity);
                if let Some(&tab) = self.tab_model.data::<PopupTab>(entity) {
                    self.select_tab(tab);
                }
            }
            Message::TemperatureUnitActivated(entity) => {
                self.temperature_model.activate(entity);
                if let Some(&unit) = self.temperature_model.data::<TemperatureUnit>(entity) {
                    self.config.temperature_unit = unit;
                    self.save_config();
                    return Self::refresh_task();
                }
            }
            Message::MeasurementActivated(entity) => {
                self.measurement_model.activate(entity);
                if let Some(&system) = self.measurement_model.data::<MeasurementSystem>(entity) {
                    self.config.measurement_system = system;
                    self.save_config();
                    return Self::refresh_task();
                }
            }
            Message::PressureUnitActivated(entity) => {
                self.pressure_model.activate(entity);
                if let Some(&unit) = self.pressure_model.data::<PressureUnit>(entity) {
                    self.config.pressure_unit = unit;
                    self.save_config();
                }
            }
            Message::SystemTimeConfig(config) => self.handle_system_time_config(config),
            Message::ShowPollutants => {
                self.showing_pollutants = true;
            }
            Message::HidePollutants => {
                self.showing_pollutants = false;
            }
            Message::PollenUpdated(gen, result) => {
                // Drop superseded results before the tri-state write, so a
                // stale result neither clobbers data nor trips the suppress-transients
                // Some(None) policy for the live request.
                if !is_current_generation(self.fetch_generation, gen) {
                    return Task::none();
                }
                match result {
                    Ok(data) => self.pollen = Some(data),
                    Err(e) => {
                        // Pollen is region-optional. Treat network failures as
                        // "no data" rather than surfacing them — the alternative
                        // is a blip of "Pollen unavailable" UI whenever the
                        // network hiccups, which is worse than no UI at all.
                        self.pollen = Some(None);
                        tracing::warn!("pollen fetch failed: {e}");
                    }
                }
            }
            Message::ShowPollen => {
                self.showing_pollen = true;
            }
            Message::HidePollen => {
                self.showing_pollen = false;
            }
            Message::ShowLocations => {
                self.showing_locations = true;
            }
            Message::HideLocations => {
                self.showing_locations = false;
            }
            Message::SwitchLocation(idx) => return self.handle_switch_location(idx),
            Message::SaveLocation(idx) => self.handle_save_location(idx),
            Message::RemoveSavedLocation(idx) => {
                if idx < self.config.saved_locations.len() {
                    self.config.saved_locations.remove(idx);
                    self.save_config();
                }
            }
            Message::OpenKofi => {
                if let Err(e) = open::that("https://ko-fi.com/vintagetechie") {
                    tracing::error!("Failed to open Ko-fi URL: {}", e);
                }
            }
            Message::OpenSourceCode => {
                if let Err(e) = open::that(
                    "https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest",
                ) {
                    tracing::error!("Failed to open source URL: {}", e);
                }
            }
            Message::OpenWorkItems => {
                if let Err(e) = open::that(
                    "https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/work_items",
                ) {
                    tracing::error!("Failed to open work items URL: {}", e);
                }
            }
            Message::RetryFetch => {
                return Self::refresh_task();
            }
            Message::NetworkChanged(crate::network::NetworkEvent::Connected) => {
                self.retry_count = 0;
                return Self::refresh_task();
            }
            Message::SystemResumed => {
                weathervane::reset_http_client();
                self.retry_count = 0;
                return Self::refresh_task();
            }
        }
        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

impl Tempest {
    /// Clears every sub-view overlay (pollutants, pollen, locations) so the
    /// selected tab renders instead of a stale overlay. Invoked on every tab
    /// selection via `select_tab`.
    fn leave_subviews(&mut self) {
        self.showing_pollutants = false;
        self.showing_pollen = false;
        self.showing_locations = false;
    }

    /// The shared tab-selection body for `SelectTab` and `TabActivated`.
    ///
    /// Sets the active tab and clears any open sub-view overlay, so a
    /// tab switch always lands on the selected tab rather than a stale overlay.
    /// Both call sites route their common work through here. The segmented-tab
    /// model is reconciled by each caller: `SelectTab` rebuilds `tab_model`
    /// after this (it has no activated entity), while `TabActivated` has already
    /// activated the entity on the existing model — so the model rebuild stays
    /// out of this helper to preserve each arm's observable behavior.
    ///
    /// `default_tab` is intentionally NOT persisted here: it is written once on
    /// popup CLOSE via `commit_pending_edits`, not per tab click.
    fn select_tab(&mut self, tab: PopupTab) {
        self.active_tab = tab;
        self.leave_subviews();
    }

    /// Commit the refresh-interval edit buffer.
    ///
    /// Parses `refresh_input`; if it is a valid `u64` in `1..=1440` it is
    /// persisted. Otherwise (non-numeric or out of range) the field REVERTS to
    /// the last persisted value — no config write, no error UI. The `1..=1440`
    /// range-check also guards the tick subscription: a 0/overflow/non-numeric
    /// interval can never be committed, so the tick cadence cannot be driven to
    /// a degenerate value.
    fn commit_refresh_interval(&mut self) {
        match self.refresh_input.parse::<u64>() {
            Ok(interval) if (1..=1440).contains(&interval) => {
                self.config.refresh_interval_minutes = interval;
                self.save_config();
            }
            _ => {
                // Revert to the last persisted value.
                self.refresh_input = self.config.refresh_interval_minutes.to_string();
            }
        }
    }

    /// Commit the AQI-token edit buffer.
    ///
    /// Mirrors the original per-keystroke trim logic: an empty (after trim)
    /// field clears the token, otherwise the trimmed value is stored. The input
    /// buffer is normalized to the trimmed value so the field reflects what was
    /// persisted.
    fn commit_aqicn_token(&mut self) {
        let trimmed = self.aqicn_token_input.trim();
        self.config.aqicn_token = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
        self.aqicn_token_input = trimmed.to_string();
        self.save_config();
    }

    /// Commit every pending edit on popup close.
    ///
    /// Persists the refresh-interval and AQI-token edit buffers and the
    /// last-used tab (`default_tab`) in a single place. Called from BOTH
    /// popup-close routes (`handle_toggle_popup` close branch and the
    /// `PopupClosed` dismiss handler) so a pending edit can never leak and the
    /// reopened tab is always the last-used one.
    fn commit_pending_edits(&mut self) {
        self.commit_refresh_interval();
        self.commit_aqicn_token();
        self.config.default_tab = self.active_tab;
        self.save_config();
    }

    /// Standard "trigger a fresh weather pull" task, used by message handlers
    /// that change anything weather-bearing (unit swaps, location changes,
    /// network/sleep wake-ups, manual retries).
    fn refresh_task() -> Task<Message> {
        Task::perform(async { Message::RefreshWeather }, Action::App)
    }

    /// One-shot async cosmic-randr resolution query, kept off the startup
    /// critical path. Fired at init and on each popup open so monitor/resolution
    /// changes are picked up on the next open; the result lands as
    /// `Message::ScreenResolution`.
    fn screen_resolution_task() -> Task<Message> {
        Task::perform(async { get_screen_resolution_async().await }, |res| {
            Action::App(Message::ScreenResolution(res))
        })
    }

    /// One-shot async geolocation lookup. Fired at init (when auto-location is
    /// enabled), from the `DetectLocation` message, and when toggling into
    /// auto-location mode; the result lands as `Message::LocationDetected`.
    fn detect_location_task() -> Task<Message> {
        Task::perform(
            async { detect_location().await.map_err(|e| e.to_string()) },
            |result| Action::App(Message::LocationDetected(result)),
        )
    }

    /// Opens the popup, or closes it if already open.
    fn handle_toggle_popup(&mut self) -> Task<Message> {
        if let Some(p) = self.popup.take() {
            // Panel-button close route — commit any pending text-field edits
            // and persist the last-used tab before tearing down the popup
            // (one of two close routes; see PopupClosed).
            self.commit_pending_edits();
            destroy_popup(p)
        } else {
            let new_id = Id::unique();
            self.popup.replace(new_id);
            let mut popup_settings = self.core.applet.get_popup_settings(
                self.core.main_window_id().unwrap(),
                new_id,
                None,
                None,
                None,
            );
            popup_settings.positioner.size_limits = self.popup_limits();
            // Re-fire the resolution query so a monitor/resolution change
            // is reflected on the next open. This open uses the cached
            // popup_max_height; the refreshed value lands before the next open.
            Task::batch([get_popup(popup_settings), Self::screen_resolution_task()])
        }
    }

    /// Fires every fetch (weather, air quality, optional alerts, optional pollen)
    /// in parallel. Pollen runs unconditionally because the API itself signals
    /// "not covered" via `Ok(None)`, see [`Message::PollenUpdated`].
    fn handle_refresh_weather(&mut self) -> Task<Message> {
        self.is_loading = true;
        self.error_message = None;

        // Bump the request generation at the single refresh entry
        // point. Every refresh path — rapid manual refresh, retry, network
        // reconnect, resume, AND the location-switch handlers — routes through
        // `refresh_task()` -> `RefreshWeather` -> here, so this one bump covers
        // them all without double-counting. `gen` is captured by value into each
        // task closure and echoed back in the result payload for the stale guard.
        self.fetch_generation += 1;
        let gen = self.fetch_generation;

        let lat = self.config.latitude;
        let lon = self.config.longitude;
        let temp_unit = self.config.temperature_unit;
        let measurement = self.config.measurement_system;
        let alerts_enabled = self.config.alerts_enabled;

        let weather_task = Task::perform(
            async move {
                fetch_weather(lat, lon, temp_unit, measurement)
                    .await
                    .map_err(|e| e.to_string())
            },
            move |result| Action::App(Message::WeatherUpdated(gen, result)),
        );

        let aqicn_token = self.config.aqicn_token.clone();
        let air_quality_task = Task::perform(
            async move {
                fetch_air_quality(lat, lon, aqicn_token.as_deref())
                    .await
                    .map_err(|e| e.to_string())
            },
            move |result| Action::App(Message::AirQualityUpdated(gen, result)),
        );

        let alerts_task = if alerts_enabled {
            Task::perform(
                async move { fetch_alerts(lat, lon).await.map_err(|e| e.to_string()) },
                move |result| Action::App(Message::AlertsUpdated(gen, result)),
            )
        } else {
            Task::none()
        };

        let pollen_task = Task::perform(
            async move { fetch_pollen(lat, lon).await.map_err(|e| e.to_string()) },
            move |result| Action::App(Message::PollenUpdated(gen, result)),
        );

        Task::batch([weather_task, air_quality_task, alerts_task, pollen_task])
    }

    /// Stores fresh weather data and updates the cached timestamp display, or
    /// schedules an exponential-backoff retry on failure.
    fn handle_weather_updated(
        &mut self,
        gen: u64,
        result: Result<WeatherData, String>,
    ) -> Task<Message> {
        // Discard a superseded result BEFORE touching
        // is_loading / retry_count / error_message, so a stale drop can never
        // reset the live request's exponential-backoff state.
        if !is_current_generation(self.fetch_generation, gen) {
            return Task::none();
        }

        self.is_loading = false;

        match result {
            Ok(data) => {
                self.retry_count = 0;
                self.current_condition = data.current.condition;
                self.display_label = self
                    .config
                    .temperature_unit
                    .format(data.current.temperature);
                self.weather_data = Some(data);
                // Series, bars, and axis all change — invalidate the cached chart.
                self.meteogram_cache.clear();
                self.error_message = None;
                // A good reading clears any stale marker from a prior failed refresh.
                self.refresh_failed = false;

                let now = chrono::Local::now();
                self.config.last_updated = Some(now.timestamp());
                self.last_updated_display = Some(self.format_time_of_day(now));
                self.save_config();
                Task::none()
            }
            Err(e) => {
                tracing::error!("Failed to fetch weather: {}", e);
                if self.weather_data.is_some() {
                    // Transient failure with a last good reading still in hand:
                    // keep the cached weather (popup and panel both) and just
                    // flag the staleness on the header, rather than wiping the
                    // display to the full error view on every network blip.
                    self.refresh_failed = true;
                } else {
                    // No data at all — surface the full error view (and the
                    // panel error glyph) since there's nothing to keep showing.
                    self.display_label = crate::fl!("panel-error");
                    self.current_condition = weathervane::WeatherCondition::Unknown;
                    self.error_message = Some(crate::fl!("weather-fetch-error"));
                }

                const BACKOFF_SECS: [u64; 4] = [5, 15, 30, 60];
                if (self.retry_count as usize) < BACKOFF_SECS.len() {
                    let delay = BACKOFF_SECS[self.retry_count as usize];
                    self.retry_count += 1;
                    tracing::info!("Scheduling retry {} in {}s", self.retry_count, delay);
                    Task::perform(
                        async move {
                            tokio::time::sleep(Duration::from_secs(delay)).await;
                            Message::RetryFetch
                        },
                        Action::App,
                    )
                } else {
                    tracing::warn!(
                        "Giving up after {} retries, waiting for next refresh",
                        self.retry_count
                    );
                    Task::none()
                }
            }
        }
    }

    /// Formats a chrono datetime using the current `military_time` setting.
    /// In 12-hour mode the leading zero on hours like "09:30 AM" is stripped.
    fn format_time_of_day(&self, dt: chrono::DateTime<chrono::Local>) -> String {
        let fmt = if self.military_time {
            "%H:%M"
        } else {
            "%I:%M %p"
        };
        let formatted = dt.format(fmt).to_string();
        if self.military_time {
            formatted
        } else {
            formatted.trim_start_matches('0').to_string()
        }
    }

    /// Swaps between auto-detect (saves current as manual fallback, kicks off
    /// detection) and manual mode (restores saved manual location and refreshes).
    fn handle_toggle_auto_location(&mut self) -> Task<Message> {
        self.config.use_auto_location = !self.config.use_auto_location;

        if self.config.use_auto_location {
            self.config.manual_latitude = Some(self.config.latitude);
            self.config.manual_longitude = Some(self.config.longitude);
            self.config.manual_location_name = Some(self.config.location_name.clone());
            self.save_config();

            Self::detect_location_task()
        } else {
            if let (Some(lat), Some(lon), Some(name)) = (
                self.config.manual_latitude,
                self.config.manual_longitude,
                self.config.manual_location_name.clone(),
            ) {
                self.config.latitude = lat;
                self.config.longitude = lon;
                self.config.location_name = name;
            }
            self.save_config();
            Self::refresh_task()
        }
    }

    /// Reformats the cached timestamp when the COSMIC time applet switches
    /// between 12h and 24h modes.
    fn handle_system_time_config(&mut self, config: TimeAppletConfig) {
        self.military_time = config.military_time;
        // Axis time labels (format_hour) change with the 12/24h preference.
        self.meteogram_cache.clear();
        if let Some(timestamp) = self.config.last_updated {
            if let Some(dt) = chrono::DateTime::from_timestamp(timestamp, 0) {
                let local = dt.with_timezone(&chrono::Local);
                self.last_updated_display = Some(self.format_time_of_day(local));
            }
        }
    }

    /// Stores a manually chosen location: sets the active coordinates/name and
    /// the persisted `manual_*` shadow fields, and disables auto-location. The
    /// single source of truth for the manual-location assignment shared by
    /// `handle_switch_location` and `handle_select_location`.
    fn set_manual_location(&mut self, lat: f64, lon: f64, name: String) {
        self.config.latitude = lat;
        self.config.longitude = lon;
        self.config.location_name = name.clone();
        self.config.manual_latitude = Some(lat);
        self.config.manual_longitude = Some(lon);
        self.config.manual_location_name = Some(name);
        self.config.use_auto_location = false;
    }

    /// Switches to a previously saved location and refreshes.
    fn handle_switch_location(&mut self, idx: usize) -> Task<Message> {
        if let Some(location) = self.config.saved_locations.get(idx) {
            let (lat, lon, name) = (location.latitude, location.longitude, location.name.clone());
            // Saved locations don't carry a structured country, so derive it from the
            // name's last comma-separated segment to keep auto-units correct after a
            // switch. This matches the behavior from before the country was a stored
            // field, when ToggleAutoUnits split the active location name. Without it,
            // the prior location's country would linger and auto-units could apply the
            // wrong country's unit rules.
            let country = name
                .split(',')
                .next_back()
                .map(|segment| segment.trim().to_string());
            self.set_manual_location(lat, lon, name);
            self.config.country = country;
            self.showing_locations = false;
            self.save_config();
            return Self::refresh_task();
        }
        Task::none()
    }

    /// Picks a location from the search-results list and refreshes.
    fn handle_select_location(&mut self, idx: usize) -> Task<Message> {
        let Some(location) = self.search_results.get(idx) else {
            return Task::none();
        };
        let country = location.country.clone();
        self.set_manual_location(
            location.latitude,
            location.longitude,
            location.display_name.clone(),
        );

        self.config.country = Some(country.clone());
        self.apply_units_for_country(&country);

        self.city_input.clear();
        self.search_results.clear();
        self.save_config();
        Self::refresh_task()
    }

    /// Bookmarks a search result into `saved_locations`, deduplicated by
    /// coordinate, capped at the configured 8-slot limit.
    fn handle_save_location(&mut self, idx: usize) {
        if let Some(location) = self.search_results.get(idx) {
            // At the 8-slot cap, surface inline feedback and PRESERVE
            // the search so the user can remove a saved location and retry —
            // rather than silently no-op'ing and clearing what they searched.
            if self.config.saved_locations.len() >= 8 {
                self.saved_locations_full = true;
                return;
            }
            let saved = crate::config::SavedLocation {
                name: location.display_name.clone(),
                latitude: location.latitude,
                longitude: location.longitude,
            };
            if !self
                .config
                .saved_locations
                .iter()
                .any(|l| l.matches_coords(saved.latitude, saved.longitude))
            {
                self.config.saved_locations.push(saved);
                self.save_config();
            }
        }
        // Successful (or under-cap dedup) save: clear the cap feedback and the
        // completed search.
        self.saved_locations_full = false;
        self.search_results.clear();
        self.city_input.clear();
    }

    /// Applies an auto-detected location and refreshes; logs the error on failure.
    fn handle_location_detected(
        &mut self,
        result: Result<DetectedLocation, String>,
    ) -> Task<Message> {
        match result {
            Ok(loc) => {
                self.config.latitude = loc.latitude;
                self.config.longitude = loc.longitude;
                self.config.location_name = loc.display_name;
                let country = loc.country;

                self.config.country = Some(country.clone());
                self.apply_units_for_country(&country);

                self.save_config();
                Self::refresh_task()
            }
            Err(e) => {
                tracing::error!("Failed to detect location: {}", e);
                Task::none()
            }
        }
    }

    fn save_config(&self) {
        if let Some(ref handler) = self.config_handler {
            if let Err(e) = self.config.write_entry(handler) {
                tracing::error!("Failed to save config: {}", e);
            }
        }
    }

    /// Sends a desktop notification for a weather alert.
    fn send_alert_notification(&self, alert: &Alert) {
        use notify_rust::{Notification, Urgency};

        let urgency = match alert.severity {
            AlertSeverity::Extreme | AlertSeverity::Severe => Urgency::Critical,
            AlertSeverity::Moderate => Urgency::Normal,
            _ => Urgency::Low,
        };

        let summary = sanitize_notification_text(&alert.event, 100);
        let body = sanitize_notification_text(&alert.headline, 300);

        if let Err(e) = Notification::new()
            .summary(&summary)
            .body(&body)
            .icon("weather-severe-alert")
            .urgency(urgency)
            .show()
        {
            tracing::warn!("Failed to send alert notification: {}", e);
        }
    }

    /// Returns the size limits for the popup window.
    fn popup_limits(&self) -> Limits {
        Limits::NONE
            .min_width(480.0)
            .max_width(480.0)
            .min_height(180.0)
            .max_height(self.popup_max_height)
    }

    /// Sets temperature and measurement units based on country if auto_units is enabled.
    fn apply_units_for_country(&mut self, country: &str) {
        if self.config.auto_units {
            if uses_imperial_units(country) {
                self.config.temperature_unit = TemperatureUnit::Fahrenheit;
                self.config.measurement_system = MeasurementSystem::Imperial;
                self.config.pressure_unit = PressureUnit::InHg;
            } else {
                self.config.temperature_unit = TemperatureUnit::Celsius;
                self.config.measurement_system = MeasurementSystem::Metric;
                self.config.pressure_unit = PressureUnit::Hpa;
            }
            // Sync the segmented control models with the new values
            self.temperature_model = build_temperature_model(self.config.temperature_unit);
            self.measurement_model = build_measurement_model(self.config.measurement_system);
            self.pressure_model = build_pressure_model(self.config.pressure_unit);
        }
    }
}

/// Strips HTML/XML tags and truncates to a maximum length.
///
/// Alert data from external APIs (NWS, MeteoAlarm, ECCC, BOM) can contain
/// markup that some notification daemons render. This neutralizes tags and
/// keeps the text to a reasonable size.
fn sanitize_notification_text(input: &str, max_len: usize) -> String {
    let mut output = String::with_capacity(input.len());
    let mut inside_tag = false;

    for ch in input.chars() {
        match ch {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => output.push(ch),
            _ => {}
        }
    }

    if output.chars().count() > max_len {
        output = output.chars().take(max_len).collect();
        output.push_str("...");
    }

    output
}

/// Returns `true` when an incoming fetch result belongs to the current request
/// generation and should be applied; `false` for any superseded (stale or
/// defensively future) result, which the caller drops via `Task::none()`.
/// Extracted as a pure helper so the generation-compare logic is unit-testable.
fn is_current_generation(current: u64, incoming: u64) -> bool {
    current == incoming
}

#[cfg(test)]
mod tests {
    use super::*;

    // Regression guard: byte-based truncation panicked when `max_len`
    // landed mid-codepoint on multibyte alert text. With char semantics this must
    // keep exactly `max_len` characters and append the "..." suffix.
    #[test]
    fn truncation_is_char_safe_for_multibyte() {
        let input = "é".repeat(100);
        let result = sanitize_notification_text(&input, 99);
        assert_eq!(result, format!("{}...", "é".repeat(99)));
        assert_eq!(result.chars().count(), 99 + 3);
    }

    // ASCII text already within `max_len` is returned verbatim — no "..." suffix.
    #[test]
    fn ascii_text_within_limit_is_unchanged() {
        let input = "Tornado warning";
        assert_eq!(sanitize_notification_text(input, 99), input);
    }

    // Markup is stripped first, then the multibyte remainder is cut on a character
    // boundary — never panics even with a small `max_len`.
    #[test]
    fn markup_and_multibyte_combined() {
        let result = sanitize_notification_text("<b>café</b>", 3);
        assert_eq!(result, "caf...");
    }

    // The pure generation-compare helper. A result is applied only
    // when its captured generation matches the current counter; any mismatch
    // (stale-incoming or defensive future-incoming) drops the result.
    #[test]
    fn current_generation_matches() {
        assert!(is_current_generation(5, 5));
    }

    #[test]
    fn stale_incoming_generation_is_dropped() {
        assert!(!is_current_generation(6, 5));
    }

    #[test]
    fn defensive_mismatch_is_dropped() {
        assert!(!is_current_generation(5, 6));
    }
}
