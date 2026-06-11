// SPDX-License-Identifier: GPL-3.0-only

use cosmic::app::{Core, Task};
use cosmic::cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry};
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::window::Id;
use cosmic::iced::{Limits, Subscription};
use cosmic::widget::{self, canvas, segmented_button, settings};
use cosmic::{Action, Application, Element};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;

use crate::config::{Config, MeasurementSystem, PopupTab, PressureUnit, TemperatureUnit};
use crate::weather::{
    aqi_to_description, categorize_pollen, condition_to_description, detect_location,
    detect_region, fetch_air_quality, fetch_alerts, fetch_pollen, fetch_weather, format_date,
    format_hour, format_time, is_night_time, pollen_level_to_description,
    pollen_species_to_description, search_city, uses_imperial_units, AirQualityData, Alert,
    AlertSeverity, AqiStandard, DetectedLocation, LocationResult, PollenData, PollenLevel,
    PollenSpecies, Region, WeatherData,
};

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
    /// Cached max popup height based on screen resolution.
    popup_max_height: f32,
    /// Consecutive fetch failures driving the backoff retry schedule.
    retry_count: u8,
    /// Monotonic request-generation counter (FIX-03 / D-08). Bumped at every
    /// logical fetch start so superseded in-flight results can be discarded.
    fetch_generation: u64,
    /// Shared tessellation cache for the meteogram canvas (PERF-01). Borrowed by
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
/// directly so it can run off the startup critical path (PERF-02 / D-07).
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
/// enabled (D-12) — disabling the meteogram removes it from the bar (D-14).
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

/// Returns the species in `data` with a non-zero, non-`OffSeason` reading,
/// paired with the raw grains/m³ value and the EAN severity bucket. Sorted
/// in the natural species order; callers pick the leader with
/// `iter().max_by_key(|(_, _, level)| *level)`.
fn active_pollen_species(data: &PollenData) -> Vec<(PollenSpecies, f32, PollenLevel)> {
    [
        (PollenSpecies::Alder, data.alder),
        (PollenSpecies::Birch, data.birch),
        (PollenSpecies::Grass, data.grass),
        (PollenSpecies::Mugwort, data.mugwort),
        (PollenSpecies::Olive, data.olive),
        (PollenSpecies::Ragweed, data.ragweed),
    ]
    .into_iter()
    .map(|(s, g)| (s, g, categorize_pollen(s, g)))
    .filter(|(_, _, level)| *level != PollenLevel::OffSeason)
    .collect()
}

/// Picks the panel text role to match the current size tier.
fn panel_text(size: cosmic::applet::Size, label: &str) -> Element<'_, Message> {
    use cosmic::applet::cosmic_panel_config::PanelSize;
    use cosmic::applet::Size;
    match size {
        Size::PanelSize(p) => match p {
            PanelSize::XL => widget::text::title1(label).into(),
            PanelSize::L => widget::text::title2(label).into(),
            PanelSize::M => widget::text::title3(label).into(),
            _ => widget::text::heading(label).into(),
        },
        _ => widget::text::heading(label).into(),
    }
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
    /// Async result of the cosmic-randr resolution query (PERF-02 / D-07).
    /// `None` => no usable display; the handler keeps the 650px fallback.
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
        // D-14: a persisted `default_tab == Graph` while the meteogram is disabled
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

        let app = Tempest {
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
            last_updated_display: None,
            showing_pollutants: false,
            showing_locations: false,
            pollen: None,
            showing_pollen: false,
            popup_max_height: POPUP_MAX_HEIGHT_FALLBACK,
            retry_count: 0,
            fetch_generation: 0,
            meteogram_cache: canvas::Cache::new(),
        };

        // Start with auto-location if enabled, otherwise fetch weather
        let task = if config.use_auto_location {
            Task::perform(
                async { detect_location().await.map_err(|e| e.to_string()) },
                |result| Action::App(Message::LocationDetected(result)),
            )
        } else {
            Task::perform(async { Message::RefreshWeather }, Action::App)
        };

        // Fire the resolution query off the startup critical path (PERF-02 / D-07):
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

    /// Dark/light theme seam (PERF-01 / D-02): the meteogram's `is_dark` branch
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
        use chrono::{Datelike, Local, Timelike};
        use cosmic::iced::Alignment;

        let spacing = cosmic::theme::spacing();

        // Determine if it's night time using actual sunrise/sunset data
        let is_night = self
            .weather_data
            .as_ref()
            .and_then(|w| {
                w.forecast
                    .first()
                    .map(|day| is_night_time(&day.sunrise, &day.sunset, w.utc_offset_seconds))
            })
            .unwrap_or_else(|| {
                // Fallback to 6pm-6am if no weather data available
                let hour = Local::now().hour();
                !(6..18).contains(&hour)
            });

        // Use error icon if there's an error, otherwise use weather icon
        let icon_name = if self.error_message.is_some() {
            "dialog-error-symbolic"
        } else {
            self.current_condition.icon_name(is_night)
        };

        let panel_icon_size = self.core.applet.suggested_size(true).0;

        let icon = widget::icon::from_name(icon_name)
            .size(panel_icon_size)
            .symbolic(true);

        let temperature_text = panel_text(self.core.applet.size.clone(), &self.display_label);

        let has_alerts = !self.alerts.is_empty();
        let alert_icon = widget::icon::from_name("dialog-warning-symbolic")
            .size(panel_icon_size)
            .symbolic(true);

        // Precompute optional panel strings once for both orientations
        let aqi_label = if self.config.show_aqi_in_panel {
            self.current_aqi
                .map(|(aqi, _)| crate::fl!("aqi-label", value = aqi))
        } else {
            None
        };

        let (dew_point_label, pressure_label, sun_label) = if let Some(weather) = &self.weather_data
        {
            let dew = if self.config.show_dew_point_in_panel {
                let s = self
                    .config
                    .temperature_unit
                    .format(weather.current.dew_point);
                Some(crate::fl!("panel-dew-point", value = s.as_str()))
            } else {
                None
            };
            let pres = if self.config.show_pressure_in_panel {
                let s = self.config.pressure_unit.format(weather.current.pressure);
                Some(crate::fl!("panel-pressure", value = s.as_str()))
            } else {
                None
            };
            let sun = if self.config.show_sunrise_sunset_in_panel {
                weather.forecast.first().map(|day| {
                    let rise = format_time(&day.sunrise, self.military_time);
                    let set = format_time(&day.sunset, self.military_time);
                    format!("{}/{}", rise, set)
                })
            } else {
                None
            };
            (dew, pres, sun)
        } else {
            (None, None, None)
        };

        let mut children: Vec<Element<'_, Message>> = Vec::new();
        if has_alerts {
            children.push(alert_icon.into());
        }
        if self.config.show_icon_in_panel {
            children.push(icon.into());
        }
        children.push(temperature_text);
        for label in [&aqi_label, &dew_point_label, &pressure_label, &sun_label]
            .into_iter()
            .flatten()
        {
            children.push(widget::text::caption(label.clone()).into());
        }

        // Panel Pride accent: shows on the default panel size (S) and larger.
        // Only the extra-small tier (and any non-`PanelSize` fallback) is treated
        // as too cramped for the bar, so the accent is skipped only there. S is
        // COSMIC's default panel size, so it must be included. The popup stripe
        // carries the nod regardless when the panel skips it.
        let roomy_tier = {
            use cosmic::applet::cosmic_panel_config::PanelSize;
            use cosmic::applet::Size;
            matches!(
                self.core.applet.size.clone(),
                Size::PanelSize(PanelSize::S | PanelSize::M | PanelSize::L | PanelSize::XL)
            )
        };
        let show_panel_pride = crate::pride::should_show_panel_accent(
            crate::pride::is_pride_month(Local::now().month()),
            self.config.pride_accent,
            roomy_tier,
        );

        let is_horizontal = self.core.applet.is_horizontal();
        let readout: Element<'_, Message> = if is_horizontal {
            widget::Row::with_children(children)
                .align_y(Alignment::Center)
                .spacing(spacing.space_xxs)
                .into()
        } else {
            widget::Column::with_children(children)
                .align_x(Alignment::Center)
                .spacing(spacing.space_xxs)
                .into()
        };

        // When the accent shows, wrap the readout with the rainbow bar on the
        // OPPOSITE axis: a horizontal readout gets an underline below it (outer
        // Column); a vertical readout gets a side-stripe beside it (outer Row).
        let data: Element<'_, Message> = if show_panel_pride {
            if is_horizontal {
                widget::Column::with_children(vec![readout, crate::pride::rainbow_bar(true, 3.0)])
                    .align_x(Alignment::Center)
                    .spacing(spacing.space_xxs)
                    .into()
            } else {
                widget::Row::with_children(vec![readout, crate::pride::rainbow_bar(false, 3.0)])
                    .align_y(Alignment::Center)
                    .spacing(spacing.space_xxs)
                    .into()
            }
        } else {
            readout
        };

        let button = widget::button::custom(data)
            .class(cosmic::theme::Button::AppletIcon)
            .on_press(Message::TogglePopup);

        widget::autosize::autosize(button, widget::Id::unique()).into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        use chrono::{Datelike, Local};

        let spacing = cosmic::theme::spacing();
        let mut column =
            widget::Column::new()
                .spacing(spacing.space_xs)
                .padding([spacing.space_xs, 0, 0, 0]);

        // Pride Month accent: a thin full-width rainbow stripe at the very top of
        // the popup. GUARANTEED to express in the fixed 480px popup whenever it is
        // June and the toggle is on — no room gating here. (3px is the one
        // intentional fixed pixel value, per the bar's thickness contract.)
        let show_pride =
            crate::pride::is_pride_month(Local::now().month()) && self.config.pride_accent;
        if show_pride {
            column = column.push(crate::pride::rainbow_bar(true, 3.0));
        }

        // Header row with timestamp and action buttons
        let has_alerts = !self.alerts.is_empty();
        let alerts_icon = if has_alerts {
            "dialog-warning-symbolic"
        } else {
            "weather-clear-symbolic"
        };

        let mut header = widget::Row::new()
            .spacing(spacing.space_xxs)
            .align_y(cosmic::iced::Alignment::Center);

        // Add timestamp if available
        if let Some(ref formatted_time) = self.last_updated_display {
            let l_updated = crate::fl!("updated", time = formatted_time.as_str());
            header = header.push(widget::text::caption(l_updated));
        }

        let refresh_btn = widget::tooltip::tooltip(
            widget::button::icon(widget::icon::from_name("view-refresh-symbolic"))
                .on_press(Message::RefreshWeather)
                .padding(spacing.space_xs),
            widget::text::body(crate::fl!("tooltip-refresh")),
            widget::tooltip::Position::Bottom,
        )
        .gap(spacing.space_xxxs);

        let alerts_btn = widget::tooltip::tooltip(
            widget::button::icon(widget::icon::from_name(alerts_icon))
                .on_press(Message::SelectTab(PopupTab::Alerts))
                .padding(spacing.space_xs),
            widget::text::body(crate::fl!("tooltip-alerts")),
            widget::tooltip::Position::Bottom,
        )
        .gap(spacing.space_xxxs);

        let settings_btn = widget::tooltip::tooltip(
            widget::button::icon(widget::icon::from_name("emblem-system-symbolic"))
                .on_press(Message::SelectTab(PopupTab::Settings))
                .padding(spacing.space_xs),
            widget::text::body(crate::fl!("tooltip-settings")),
            widget::tooltip::Position::Bottom,
        )
        .gap(spacing.space_xxxs);

        header = header
            .push(widget::space::horizontal())
            .push(refresh_btn)
            .push(alerts_btn)
            .push(settings_btn);

        column = column.push(header);

        // Prominent location display (tappable when saved locations exist)
        if self.config.saved_locations.len() > 1 {
            column = column.push(
                widget::container(
                    widget::button::custom(
                        widget::Row::new()
                            .spacing(spacing.space_xxs)
                            .align_y(cosmic::iced::Alignment::Center)
                            .push(widget::text::title4(&self.config.location_name))
                            .push(widget::icon::from_name("go-next-symbolic").size(16)),
                    )
                    .class(cosmic::theme::Button::Text)
                    .on_press(Message::ShowLocations),
                )
                .align_x(cosmic::iced::alignment::Horizontal::Center)
                .width(cosmic::iced::Length::Fill),
            );
        } else {
            column = column.push(
                widget::container(widget::text::title4(&self.config.location_name))
                    .align_x(cosmic::iced::alignment::Horizontal::Center)
                    .width(cosmic::iced::Length::Fill),
            );
        }

        column = column.push(widget::divider::horizontal::default());

        // Show error message if there is one
        if let Some(ref error) = self.error_message {
            column = column.push(
                widget::container(
                    widget::Column::new()
                        .spacing(spacing.space_xs)
                        .push(widget::icon::from_name("dialog-error-symbolic").size(48))
                        .push(widget::text::title4(crate::fl!("failed-to-load")))
                        .push(widget::text::body(error).width(cosmic::iced::Length::Fill))
                        .push(
                            widget::button::standard(crate::fl!("retry"))
                                .on_press(Message::RefreshWeather),
                        ),
                )
                .align_x(cosmic::iced::alignment::Horizontal::Center)
                .width(cosmic::iced::Length::Fill),
            );
        } else if self.is_loading {
            column = column.push(
                widget::container(
                    widget::Column::new()
                        .spacing(spacing.space_xs)
                        .align_x(cosmic::iced::alignment::Horizontal::Center)
                        .push(widget::icon::from_name("content-loading-symbolic").size(48))
                        .push(widget::text::title4(crate::fl!("loading"))),
                )
                .align_x(cosmic::iced::alignment::Horizontal::Center)
                .width(cosmic::iced::Length::Fill),
            );
        } else if self.showing_locations {
            // Saved locations sub-view replaces normal popup content
            column = column.push(self.render_locations_view());
        } else if self.showing_pollutants {
            // Pollutants sub-view replaces normal popup content
            column = column.push(self.render_pollutants_view());
        } else if self.showing_pollen {
            // Pollen sub-view replaces normal popup content
            column = column.push(self.render_pollen_view());
        } else if let Some(ref weather) = self.weather_data {
            let tab_control = widget::tab_bar::horizontal(&self.tab_model)
                .button_alignment(cosmic::iced::Alignment::Center)
                .on_activate(Message::TabActivated);

            column = column.push(cosmic::applet::padded_control(tab_control));

            // Tab content - delegated to helper methods
            match self.active_tab {
                PopupTab::Current => column = column.push(self.render_current_tab(weather)),
                PopupTab::Alerts => column = column.push(self.render_alerts_tab()),
                PopupTab::Hourly => column = column.push(self.render_hourly_tab(weather)),
                PopupTab::Forecast => column = column.push(self.render_forecast_tab(weather)),
                PopupTab::Graph => column = column.push(self.render_graph_tab(weather)),
                PopupTab::Settings => column = column.push(self.render_settings_tab()),
            }
        }

        let padded = widget::container(column).padding([
            0,
            spacing.space_l,
            spacing.space_l,
            spacing.space_l,
        ]);
        let scrollable = widget::scrollable(padded).height(cosmic::iced::Length::Shrink);

        self.core
            .applet
            .popup_container(scrollable)
            .limits(self.popup_limits())
            .into()
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::TogglePopup => return self.handle_toggle_popup(),
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                    self.showing_pollutants = false;
                    self.showing_pollen = false;
                    self.showing_locations = false;
                }
            }
            Message::ScreenResolution(res) => {
                // Latest-wins: a height value needs no fetch_generation guard (D-08).
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
                // FIX-03: drop superseded results before touching any state.
                if !is_current_generation(self.fetch_generation, gen) {
                    return Task::none();
                }
                match result {
                    Ok(data) => {
                        self.current_aqi = Some((data.aqi, data.standard()));
                        self.air_quality = Some(data);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch air quality: {}", e);
                        self.current_aqi = None;
                        self.air_quality = None;
                    }
                }
            }
            Message::AlertsUpdated(gen, result) => {
                // FIX-03: drop superseded results before seen_alert_ids insertion,
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
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch alerts: {}", e);
                    }
                }
            }
            Message::Tick => {
                self.retry_count = 0;
                // Hourly wall-clock advance (D-01): refresh now-marker, night
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
                    // Extract country from location name (last part after comma)
                    if let Some(country) = self.config.location_name.split(',').next_back() {
                        let country = country.trim().to_string();
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
                // D-14: disabling the meteogram removes the Graph segment, so a
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
            Message::UpdateRefreshInterval(value) => {
                self.refresh_input = value.clone();
                if let Ok(interval) = value.parse::<u64>() {
                    if (1..=1440).contains(&interval) {
                        self.config.refresh_interval_minutes = interval;
                        self.save_config();
                    }
                }
            }
            Message::UpdateAqicnToken(value) => {
                self.aqicn_token_input = value.clone();
                let trimmed = value.trim();
                self.config.aqicn_token = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                };
                self.save_config();
            }
            Message::ToggleAutoLocation => return self.handle_toggle_auto_location(),
            Message::DetectLocation => {
                return Task::perform(
                    async { detect_location().await.map_err(|e| e.to_string()) },
                    |result| Action::App(Message::LocationDetected(result)),
                );
            }
            Message::LocationDetected(result) => return self.handle_location_detected(result),
            Message::SelectTab(tab) => {
                self.active_tab = tab;
                self.config.default_tab = tab;
                self.leave_subviews();
                self.tab_model =
                    build_tab_model(tab_for_segmented_control(tab), self.config.show_meteogram);
                self.save_config();
            }
            Message::TabActivated(entity) => {
                self.tab_model.activate(entity);
                if let Some(&tab) = self.tab_model.data::<PopupTab>(entity) {
                    self.active_tab = tab;
                    self.config.default_tab = tab;
                    self.leave_subviews();
                    self.save_config();
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
                // FIX-03: drop superseded results before the tri-state write, so a
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
            Message::SwitchLocation(idx) => {
                if let Some(location) = self.config.saved_locations.get(idx) {
                    self.config.latitude = location.latitude;
                    self.config.longitude = location.longitude;
                    self.config.location_name = location.name.clone();
                    self.config.manual_latitude = Some(location.latitude);
                    self.config.manual_longitude = Some(location.longitude);
                    self.config.manual_location_name = Some(location.name.clone());
                    self.config.use_auto_location = false;
                    self.showing_locations = false;
                    self.save_config();
                    return Self::refresh_task();
                }
            }
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

const UG_PER_M3: &str = "µg/m³";

impl Tempest {
    /// Clears every sub-view overlay (pollutants, pollen, locations) so the
    /// selected tab renders instead of a stale overlay. Shared by `SelectTab`
    /// and `TabActivated`; a later DRY pass folds it into a single
    /// tab-selection helper.
    fn leave_subviews(&mut self) {
        self.showing_pollutants = false;
        self.showing_pollen = false;
        self.showing_locations = false;
    }

    /// Standard "trigger a fresh weather pull" task, used by message handlers
    /// that change anything weather-bearing (unit swaps, location changes,
    /// network/sleep wake-ups, manual retries).
    fn refresh_task() -> Task<Message> {
        Task::perform(async { Message::RefreshWeather }, Action::App)
    }

    /// One-shot async cosmic-randr resolution query (PERF-02 / D-07, D-08).
    /// Fired at init and on each popup open so monitor/resolution changes are
    /// picked up on the next open; the result lands as `Message::ScreenResolution`.
    fn screen_resolution_task() -> Task<Message> {
        Task::perform(async { get_screen_resolution_async().await }, |res| {
            Action::App(Message::ScreenResolution(res))
        })
    }

    /// Opens the popup, or closes it if already open.
    fn handle_toggle_popup(&mut self) -> Task<Message> {
        if let Some(p) = self.popup.take() {
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
            // D-08: re-fire the resolution query so a monitor/resolution change
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

        // FIX-03 / D-08: bump the request generation at the single refresh entry
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
        // FIX-03 guardrail: discard a superseded result BEFORE touching
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

                let now = chrono::Local::now();
                self.config.last_updated = Some(now.timestamp());
                self.last_updated_display = Some(self.format_time_of_day(now));
                self.save_config();
                Task::none()
            }
            Err(e) => {
                tracing::error!("Failed to fetch weather: {}", e);
                self.display_label = crate::fl!("panel-error");
                self.current_condition = weathervane::WeatherCondition::Unknown;
                self.error_message = Some(crate::fl!("weather-fetch-error"));

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

            Task::perform(
                async { detect_location().await.map_err(|e| e.to_string()) },
                |result| Action::App(Message::LocationDetected(result)),
            )
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

    /// Picks a location from the search-results list and refreshes.
    fn handle_select_location(&mut self, idx: usize) -> Task<Message> {
        let Some(location) = self.search_results.get(idx) else {
            return Task::none();
        };
        let country = location.country.clone();
        self.config.latitude = location.latitude;
        self.config.longitude = location.longitude;
        self.config.location_name = location.display_name.clone();
        self.config.use_auto_location = false;
        self.config.manual_latitude = Some(location.latitude);
        self.config.manual_longitude = Some(location.longitude);
        self.config.manual_location_name = Some(location.display_name.clone());

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
            if self.config.saved_locations.len() < 8 {
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
        }
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

    /// Creates a stat cell with label and bold value stacked vertically.
    fn stat_cell(label: String, value: String) -> Element<'static, Message> {
        let spacing = cosmic::theme::spacing();
        widget::Column::new()
            .spacing(spacing.space_xxxs)
            .push(widget::text::caption(label))
            .push(widget::text::heading(value))
            .width(cosmic::iced::Length::FillPortion(1))
            .into()
    }

    /// Creates a row with two stat cells.
    fn stat_row(
        left_label: String,
        left_value: String,
        right_label: String,
        right_value: String,
    ) -> Element<'static, Message> {
        widget::Row::new()
            .push(Self::stat_cell(left_label, left_value))
            .push(Self::stat_cell(right_label, right_value))
            .into()
    }

    /// Renders the Current weather tab content.
    fn render_current_tab(&self, weather: &WeatherData) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let mut col = widget::Column::new().spacing(spacing.space_s);

        // Temperature and condition grouped together
        col = col.push(
            widget::Column::new()
                .spacing(spacing.space_xxxs)
                .push(widget::text::title1(
                    self.config
                        .temperature_unit
                        .format(weather.current.temperature),
                ))
                .push(widget::text::body(condition_to_description(
                    weather.current.condition,
                ))),
        );

        col = col.push(widget::divider::horizontal::default());

        // Stats table
        let wind_unit = self.config.measurement_system.wind_speed_unit();
        let wind_dir = weather.current.compass_direction.as_str();
        let visibility = self
            .config
            .measurement_system
            .convert_visibility(weather.current.visibility);
        let visibility_unit = self.config.measurement_system.visibility_unit();

        // Feels like / Humidity
        col = col.push(Self::stat_row(
            crate::fl!("label-feels-like"),
            format!(
                "{:.0}{}",
                weather.current.feels_like,
                self.config.temperature_unit.symbol()
            ),
            crate::fl!("label-humidity"),
            format!("{}%", weather.current.humidity),
        ));

        // Wind / Gusts
        col = col.push(Self::stat_row(
            crate::fl!("label-wind"),
            format!(
                "{:.1} {} {}",
                weather.current.windspeed, wind_unit, wind_dir
            ),
            crate::fl!("label-gusts"),
            format!("{:.1} {}", weather.current.wind_gusts, wind_unit),
        ));

        // UV Index / Cloud cover
        col = col.push(Self::stat_row(
            crate::fl!("label-uv-index"),
            format!("{:.1}", weather.current.uv_index),
            crate::fl!("label-cloud-cover"),
            format!("{}%", weather.current.cloud_cover),
        ));

        // Visibility / Pressure
        col = col.push(Self::stat_row(
            crate::fl!("label-visibility"),
            format!("{:.1} {}", visibility, visibility_unit),
            crate::fl!("label-pressure"),
            self.config.pressure_unit.format(weather.current.pressure),
        ));

        // Sunrise / Sunset
        if let Some(first_day) = weather.forecast.first() {
            col = col.push(Self::stat_row(
                crate::fl!("label-sunrise"),
                format_time(&first_day.sunrise, self.military_time),
                crate::fl!("label-sunset"),
                format_time(&first_day.sunset, self.military_time),
            ));
        }

        // Air Quality row with chevron to open pollutants sub-view
        if let Some(ref aq) = self.air_quality {
            col = col.push(widget::divider::horizontal::default());

            let aqi_description = aqi_to_description(&aq.category);
            let aqi_content = widget::Row::new()
                .align_y(cosmic::iced::Alignment::Center)
                .push(
                    widget::Column::new()
                        .push(widget::text::title4(format!(
                            "{} {}",
                            aq.aqi, aqi_description
                        )))
                        .push(widget::text::caption(crate::fl!("air-quality-index"))),
                )
                .push(widget::space::horizontal())
                .push(widget::icon::from_name("go-next-symbolic").size(16));

            col = col.push(
                widget::list_column()
                    .add(widget::list::button(aqi_content).on_press(Message::ShowPollutants)),
            );

            // aqicn attribution. Required by their terms when their data is
            // what we're showing. Mirrors the library's selection logic:
            // non-empty token and not in Europe.
            let token_set = self
                .config
                .aqicn_token
                .as_deref()
                .map(|t| !t.trim().is_empty())
                .unwrap_or(false);
            let region = detect_region(self.config.latitude, self.config.longitude);
            if token_set && region != Region::Europe {
                col = col.push(widget::text::caption(crate::fl!("aqicn-attribution")));
            }
        } else {
            col = col.push(widget::divider::horizontal::default());
            col = col.push(widget::text::body(crate::fl!("air-quality-unavailable")));
        }

        // Pollen row. Suppressed entirely when there's no CAMS coverage
        // (Some(None)), no data yet (None), or every species is OffSeason.
        // Otherwise: lead with the highest-severity active species, caption
        // counts the rest.
        if let Some(Some(ref p)) = self.pollen {
            let active = active_pollen_species(p);
            if let Some((lead_species, _, lead_level)) = active.iter().max_by_key(|(_, _, l)| *l) {
                col = col.push(widget::divider::horizontal::default());

                let headline = format!(
                    "{} {}",
                    pollen_species_to_description(*lead_species),
                    pollen_level_to_description(*lead_level),
                );
                let caption = if active.len() > 1 {
                    crate::fl!("pollen-caption-others", n = (active.len() - 1).to_string())
                } else {
                    crate::fl!("label-pollen")
                };

                let pollen_content = widget::Row::new()
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(
                        widget::Column::new()
                            .push(widget::text::title4(headline))
                            .push(widget::text::caption(caption)),
                    )
                    .push(widget::space::horizontal())
                    .push(widget::icon::from_name("go-next-symbolic").size(16));

                col = col.push(
                    widget::list_column()
                        .add(widget::list::button(pollen_content).on_press(Message::ShowPollen)),
                );
            }
        }

        col.into()
    }

    /// Header for sub-views: back arrow on the left, title following.
    fn subview_header(title: String, on_back: Message) -> Element<'static, Message> {
        let spacing = cosmic::theme::spacing();
        widget::Row::new()
            .align_y(cosmic::iced::Alignment::Center)
            .spacing(spacing.space_xs)
            .push(
                widget::button::icon(widget::icon::from_name("go-previous-symbolic"))
                    .padding(spacing.space_xxs)
                    .on_press(on_back),
            )
            .push(widget::text::title4(title))
            .into()
    }

    /// Renders the pollutants sub-view with a close header and pollutant list.
    fn render_pollutants_view(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let mut col = widget::Column::new().spacing(spacing.space_m);

        col = col.push(Self::subview_header(
            crate::fl!("air-quality-index"),
            Message::HidePollutants,
        ));

        // Pollutant list
        if let Some(ref aq) = self.air_quality {
            let list = widget::list_column()
                .add(Self::pollutant_row(
                    crate::fl!("label-co"),
                    format!("{:.1} {}", aq.carbon_monoxide, UG_PER_M3),
                ))
                .add(Self::pollutant_row(
                    crate::fl!("label-no2"),
                    format!("{:.1} {}", aq.nitrogen_dioxide, UG_PER_M3),
                ))
                .add(Self::pollutant_row(
                    crate::fl!("label-ozone"),
                    format!("{:.1} {}", aq.ozone, UG_PER_M3),
                ))
                .add(Self::pollutant_row(
                    crate::fl!("label-pm10"),
                    format!("{:.1} {}", aq.pm10, UG_PER_M3),
                ))
                .add(Self::pollutant_row(
                    crate::fl!("label-pm25"),
                    format!("{:.1} {}", aq.pm2_5, UG_PER_M3),
                ));
            col = col.push(list);
        }

        col.into()
    }

    /// Creates a row for a pollutant with label on left and value on right.
    fn pollutant_row(label: String, value: String) -> Element<'static, Message> {
        widget::Row::new()
            .width(cosmic::iced::Length::Fill)
            .push(widget::text::body(label))
            .push(widget::space::horizontal())
            .push(widget::text::body(value))
            .into()
    }

    /// Renders the pollen sub-view: one row per species with the EAN level on
    /// the right, plus a CAMS attribution footer. OffSeason species stay in
    /// the list so users see the full landscape — they are dimmed and show
    /// "Off season" instead of a numeric reading.
    fn render_pollen_view(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let mut col = widget::Column::new().spacing(spacing.space_m);

        col = col.push(Self::subview_header(
            crate::fl!("label-pollen"),
            Message::HidePollen,
        ));

        if let Some(Some(ref p)) = self.pollen {
            let rows = [
                (PollenSpecies::Alder, p.alder),
                (PollenSpecies::Birch, p.birch),
                (PollenSpecies::Grass, p.grass),
                (PollenSpecies::Mugwort, p.mugwort),
                (PollenSpecies::Olive, p.olive),
                (PollenSpecies::Ragweed, p.ragweed),
            ];

            let mut list = widget::list_column();
            for (species, grains) in rows {
                let level = categorize_pollen(species, grains);
                let value = if level == PollenLevel::OffSeason {
                    pollen_level_to_description(level)
                } else {
                    format!("{} ({:.1})", pollen_level_to_description(level), grains)
                };
                list = list.add(Self::pollutant_row(
                    pollen_species_to_description(species),
                    value,
                ));
            }
            col = col.push(list);

            col = col.push(widget::text::caption(crate::fl!("pollen-attribution")));
        }

        col.into()
    }

    /// Renders the saved locations sub-view with a close header and location list.
    fn render_locations_view(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let mut col = widget::Column::new().spacing(spacing.space_xxs);

        col = col.push(Self::subview_header(
            crate::fl!("section-saved-locations"),
            Message::HideLocations,
        ));
        col = col.push(widget::divider::horizontal::default());

        let mut list = widget::list_column();
        for (idx, location) in self.config.saved_locations.iter().enumerate() {
            let is_active = location.matches_coords(self.config.latitude, self.config.longitude);

            list = list.add(
                widget::list::button(widget::text::body(&location.name))
                    .on_press(Message::SwitchLocation(idx))
                    .selected(is_active),
            );
        }
        col = col.push(list);

        col.into()
    }

    /// Renders the Alerts tab content.
    fn render_alerts_tab(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let mut col = widget::Column::new().spacing(spacing.space_xxs);

        if !self.config.alerts_enabled {
            col = col.push(
                widget::container(
                    widget::Column::new()
                        .spacing(spacing.space_xs)
                        .align_x(cosmic::iced::alignment::Horizontal::Center)
                        .push(widget::text::body(crate::fl!("alerts-disabled")))
                        .push(widget::text::caption(crate::fl!("alerts-enable-hint"))),
                )
                .align_x(cosmic::iced::alignment::Horizontal::Center)
                .width(cosmic::iced::Length::Fill),
            );
        } else if self.alerts.is_empty() {
            col = col.push(
                widget::container(
                    widget::Column::new()
                        .spacing(spacing.space_xs)
                        .align_x(cosmic::iced::alignment::Horizontal::Center)
                        .push(
                            widget::icon::from_name("weather-clear-symbolic")
                                .size(48)
                                .symbolic(true),
                        )
                        .push(widget::text::title4(crate::fl!("no-active-alerts")))
                        .push(widget::text::caption(crate::fl!("area-clear"))),
                )
                .align_x(cosmic::iced::alignment::Horizontal::Center)
                .width(cosmic::iced::Length::Fill),
            );
        } else {
            let mut list = widget::list_column();
            for alert in &self.alerts {
                let severity_icon = match alert.severity {
                    AlertSeverity::Extreme => "dialog-error-symbolic",
                    AlertSeverity::Severe => "dialog-warning-symbolic",
                    AlertSeverity::Moderate => "dialog-information-symbolic",
                    _ => "weather-severe-alert-symbolic",
                };

                list =
                    list.add(
                        widget::Column::new()
                            .spacing(spacing.space_xxxs)
                            .push(
                                widget::Row::new()
                                    .spacing(spacing.space_xxs)
                                    .push(
                                        widget::icon::from_name(severity_icon)
                                            .size(16)
                                            .symbolic(true),
                                    )
                                    .push(widget::text::body(&alert.event)),
                            )
                            .push(widget::text::caption(&alert.headline))
                            .push_maybe(if alert.description.is_empty() {
                                None
                            } else {
                                Some(
                                    widget::container(
                                        widget::scrollable(
                                            widget::container(widget::text::caption(
                                                &alert.description,
                                            ))
                                            .padding([0, spacing.space_s, 0, 0]),
                                        )
                                        .height(cosmic::iced::Length::Shrink),
                                    )
                                    .padding([spacing.space_xxxs, 0, spacing.space_xxxs, 0])
                                    .max_height(160.0),
                                )
                            })
                            .push({
                                let time_fmt = if self.military_time {
                                    "%b %d %H:%M"
                                } else {
                                    "%b %d %I:%M %p"
                                };
                                let expires_time = alert.expires.format(time_fmt).to_string();
                                widget::text::caption(crate::fl!(
                                    "expires",
                                    time = expires_time.as_str()
                                ))
                            }),
                    );
            }
            col = col.push(list);
        }

        col.into()
    }

    /// Renders the Hourly forecast tab content.
    fn render_hourly_tab(&self, weather: &WeatherData) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let mut col = widget::Column::new().spacing(spacing.space_xxs);
        let hours_per_row = 4;

        for chunk in weather.hourly.chunks(hours_per_row) {
            let mut row = widget::Row::new().spacing(spacing.space_xxs);

            // Wind unit (km/h or mph) matches the Current tab derivation; precipitation
            // amount unit (mm or in) is derived from the measurement system the same way.
            let wind_unit = self.config.measurement_system.wind_speed_unit();
            let precip_unit = match self.config.measurement_system {
                MeasurementSystem::Imperial => "in",
                MeasurementSystem::Metric => "mm",
            };

            for hour in chunk {
                let cell = widget::Column::new()
                    .spacing(spacing.space_xxxs)
                    .align_x(cosmic::iced::alignment::Horizontal::Center)
                    .push(widget::text::caption(format_hour(
                        &hour.time,
                        self.military_time,
                    )))
                    .push(
                        widget::icon::from_name(hour.condition.icon_name(false))
                            .size(24)
                            .symbolic(true),
                    )
                    .push(widget::text::body(
                        self.config.temperature_unit.format(hour.temperature),
                    ))
                    .push(widget::text::caption(format!(
                        "{}%",
                        hour.precipitation_probability
                    )))
                    // HOUR-01: per-hour wind speed (weathervane 0.5, already in user's unit)
                    .push(widget::text::caption(format!(
                        "{:.0} {wind_unit}",
                        hour.windspeed
                    )))
                    // HOUR-02: per-hour precipitation amount (weathervane 0.5, user's unit)
                    .push(widget::text::caption(format!(
                        "{:.1} {precip_unit}",
                        hour.precipitation
                    )));

                row = row.push(
                    widget::container(cell)
                        .width(cosmic::iced::Length::FillPortion(1))
                        .align_x(cosmic::iced::alignment::Horizontal::Center),
                );
            }

            // Pad incomplete rows
            for _ in chunk.len()..hours_per_row {
                row = row.push(widget::Space::new().width(cosmic::iced::Length::FillPortion(1)));
            }

            col = col.push(row);
        }

        col.into()
    }

    /// Renders the 7-day Forecast tab content.
    /// Renders the Graph tab: the YR.no-style meteogram canvas (GRAPH-01).
    ///
    /// The canvas is constructed against `meteogram::Meteogram`'s LOCKED field
    /// contract (`hourly` / `daily` / `military_time`; `&Vec<T>` coerces to the
    /// struct's `&[T]` fields). Height MUST be a `Fixed` value — `Shrink` collapses
    /// the canvas to zero inside the surrounding `scrollable` (Pitfall 1); 300px
    /// matches the meteogram's band-height constants (grown from 260px so the panels
    /// and time labels aren't cramped). Width fills the ~416px popup content area.
    fn render_graph_tab<'a>(&'a self, weather: &'a WeatherData) -> Element<'a, Message> {
        // Precip peak-label unit, derived the same way as the enriched Hourly cell.
        let precip_unit = match self.config.measurement_system {
            MeasurementSystem::Imperial => "in",
            MeasurementSystem::Metric => "mm",
        };
        cosmic::widget::Canvas::new(crate::meteogram::Meteogram {
            cache: &self.meteogram_cache,
            hourly: &weather.hourly,
            daily: &weather.forecast,
            military_time: self.military_time,
            precip_unit,
        })
        .width(cosmic::iced::Length::Fill)
        .height(cosmic::iced::Length::Fixed(300.0))
        .into()
    }

    fn render_forecast_tab(&self, weather: &WeatherData) -> Element<'_, Message> {
        const COL_DAY: cosmic::iced::Length = cosmic::iced::Length::FillPortion(3);
        const COL_ICON: cosmic::iced::Length = cosmic::iced::Length::Fixed(24.0);
        const COL_HIGH: cosmic::iced::Length = cosmic::iced::Length::FillPortion(1);
        const COL_LOW: cosmic::iced::Length = cosmic::iced::Length::FillPortion(1);
        const COL_COND: cosmic::iced::Length = cosmic::iced::Length::FillPortion(2);

        let spacing = cosmic::theme::spacing();
        let mut col = widget::Column::new()
            .spacing(spacing.space_xxs)
            .width(cosmic::iced::Length::Fill);

        // Table header
        col = col.push(
            widget::Row::new()
                .spacing(spacing.space_xxs)
                .align_y(cosmic::iced::Alignment::Center)
                .padding([0, spacing.space_xxs])
                .push(
                    widget::container(widget::text::heading(crate::fl!("forecast-day")))
                        .width(COL_DAY),
                )
                .push(widget::Space::new().width(COL_ICON))
                .push(
                    widget::container(widget::text::heading(crate::fl!("forecast-high")))
                        .width(COL_HIGH),
                )
                .push(
                    widget::container(widget::text::heading(crate::fl!("forecast-low")))
                        .width(COL_LOW),
                )
                .push(
                    widget::container(widget::text::heading(crate::fl!("forecast-conditions")))
                        .width(COL_COND),
                ),
        );
        col = col.push(widget::divider::horizontal::default());

        // Data rows
        for day in &weather.forecast {
            col = col.push(
                widget::Row::new()
                    .spacing(spacing.space_xxs)
                    .align_y(cosmic::iced::Alignment::Center)
                    .padding([0, spacing.space_xxs])
                    .push(
                        widget::container(widget::text::body(format_date(&day.date)))
                            .width(COL_DAY),
                    )
                    .push(
                        widget::container(
                            widget::icon::from_name(day.condition.icon_name(false))
                                .size(24)
                                .symbolic(true),
                        )
                        .width(COL_ICON)
                        .align_x(cosmic::iced::alignment::Horizontal::Center),
                    )
                    .push(
                        widget::container(widget::text::body(
                            self.config.temperature_unit.format(day.temp_max),
                        ))
                        .width(COL_HIGH),
                    )
                    .push(
                        widget::container(widget::text::body(
                            self.config.temperature_unit.format(day.temp_min),
                        ))
                        .width(COL_LOW),
                    )
                    .push(
                        widget::container(
                            widget::text::body(condition_to_description(day.condition))
                                .wrapping(cosmic::iced::widget::text::Wrapping::None)
                                .ellipsize(cosmic::iced::widget::text::Ellipsize::End(
                                    cosmic::iced::core::text::EllipsizeHeightLimit::Lines(1),
                                )),
                        )
                        .width(COL_COND),
                    ),
            );
        }

        col.into()
    }

    /// Renders the Settings tab content.
    fn render_settings_tab(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let mut col = widget::Column::new().spacing(spacing.space_xs);

        col = col.push(self.render_location_section());
        if let Some(saved) = self.render_saved_locations_section() {
            col = col.push(saved);
        }
        col = col.push(self.render_units_section());
        col = col.push(widget::text::caption(crate::fl!(
            "settings-auto-units-hint"
        )));
        col = col.push(self.render_updates_section());
        col = col.push(self.render_aq_section());
        col = col.push(self.render_panel_display_section());
        col = col.push(self.render_support_section());

        col.into()
    }

    /// LOCATION section: auto-detect toggle plus either the detected-location
    /// status row (auto on) or the manual search input and result list (auto off).
    fn render_location_section(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let mut section = settings::section()
            .title(crate::fl!("section-location"))
            .add(settings::item(
                crate::fl!("settings-auto-detect"),
                widget::toggler(self.config.use_auto_location)
                    .on_toggle(|_| Message::ToggleAutoLocation),
            ));

        if self.config.use_auto_location {
            section = section.add(
                widget::Row::new()
                    .spacing(spacing.space_xxs)
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(
                        widget::Column::new()
                            .push(widget::text::body(&self.config.location_name))
                            .push(widget::text::caption(crate::fl!("detected-via-ip")))
                            .width(cosmic::iced::Length::Fill),
                    )
                    .push(
                        widget::button::standard(crate::fl!("settings-refresh"))
                            .on_press(Message::DetectLocation),
                    ),
            );
        } else {
            section = section.add(
                widget::Row::new()
                    .spacing(spacing.space_xxs)
                    .push(
                        widget::text_input(
                            crate::fl!("settings-search-placeholder"),
                            &self.city_input,
                        )
                        .on_input(Message::UpdateCityInput)
                        .on_submit(|_| Message::SearchCity)
                        .width(cosmic::iced::Length::Fill),
                    )
                    .push(
                        widget::button::standard(crate::fl!("settings-search"))
                            .on_press(Message::SearchCity),
                    ),
            );

            for (idx, result) in self.search_results.iter().enumerate() {
                let save_btn = widget::tooltip::tooltip(
                    widget::button::icon(widget::icon::from_name("bookmark-new-symbolic").size(16))
                        .on_press(Message::SaveLocation(idx))
                        .padding(spacing.space_xxs),
                    widget::text::body(crate::fl!("tooltip-save-location")),
                    widget::tooltip::Position::Left,
                )
                .gap(spacing.space_xxxs);
                section = section.add(
                    widget::Row::new()
                        .spacing(spacing.space_xxxs)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(
                            widget::button::text(&result.display_name)
                                .on_press(Message::SelectLocation(idx))
                                .padding(spacing.space_xxs)
                                .width(cosmic::iced::Length::Fill),
                        )
                        .push(save_btn),
                );
            }

            section = section.add(
                widget::Column::new()
                    .push(widget::text::body(&self.config.location_name))
                    .push(widget::text::caption(crate::fl!("manually-selected"))),
            );
        }

        section.into()
    }

    /// SAVED LOCATIONS section. Returns None when the list is empty so the
    /// section is hidden entirely rather than rendered as an empty container.
    fn render_saved_locations_section(&self) -> Option<Element<'_, Message>> {
        if self.config.saved_locations.is_empty() {
            return None;
        }
        let spacing = cosmic::theme::spacing();
        let mut list = widget::list_column();
        for (idx, location) in self.config.saved_locations.iter().enumerate() {
            let is_active = (location.latitude - self.config.latitude).abs() < 0.01
                && (location.longitude - self.config.longitude).abs() < 0.01;

            let mut row = widget::Row::new()
                .spacing(spacing.space_xxs)
                .align_y(cosmic::iced::Alignment::Center)
                .push(widget::text::body(&location.name).width(cosmic::iced::Length::Fill));

            if is_active {
                row = row.push(widget::icon::from_name("emblem-ok-symbolic").size(16));
            }

            let remove_btn = widget::tooltip::tooltip(
                widget::button::icon(widget::icon::from_name("edit-delete-symbolic").size(16))
                    .on_press(Message::RemoveSavedLocation(idx))
                    .padding(spacing.space_xxxs),
                widget::text::body(crate::fl!("tooltip-remove-saved-location")),
                widget::tooltip::Position::Left,
            )
            .gap(spacing.space_xxxs);
            row = row.push(remove_btn);

            list = list.add(row);
        }
        Some(
            settings::section()
                .title(crate::fl!("section-saved-locations"))
                .add(list)
                .into(),
        )
    }

    /// UNITS section: temperature, measurement, and pressure pickers plus the
    /// "auto-select by location" toggle.
    fn render_units_section(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let temperature_row = widget::Column::new()
            .spacing(spacing.space_xxs)
            .push(widget::text::body(crate::fl!("settings-temperature")))
            .push(
                widget::segmented_control::horizontal(&self.temperature_model)
                    .on_activate(Message::TemperatureUnitActivated),
            );

        let measurement_row = widget::Column::new()
            .spacing(spacing.space_xxs)
            .push(widget::text::body(crate::fl!("settings-measurement")))
            .push(
                widget::segmented_control::horizontal(&self.measurement_model)
                    .on_activate(Message::MeasurementActivated),
            );

        let pressure_row = widget::Column::new()
            .spacing(spacing.space_xxs)
            .push(widget::text::body(crate::fl!("settings-pressure")))
            .push(
                widget::segmented_control::horizontal(&self.pressure_model)
                    .on_activate(Message::PressureUnitActivated),
            );

        settings::section()
            .title(crate::fl!("section-units"))
            .add(temperature_row)
            .add(measurement_row)
            .add(pressure_row)
            .add(settings::item(
                crate::fl!("settings-auto-units"),
                widget::toggler(self.config.auto_units).on_toggle(|_| Message::ToggleAutoUnits),
            ))
            .into()
    }

    /// UPDATES section: refresh interval and weather-alerts toggle.
    fn render_updates_section(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        settings::section()
            .title(crate::fl!("section-updates"))
            .add(settings::item(
                crate::fl!("settings-refresh-interval"),
                widget::Row::new()
                    .spacing(spacing.space_xxs)
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(widget::text::body(crate::fl!("settings-min")))
                    .push(
                        widget::text_input("15", &self.refresh_input)
                            .on_input(Message::UpdateRefreshInterval),
                    ),
            ))
            .add(settings::item(
                crate::fl!("settings-weather-alerts"),
                widget::toggler(self.config.alerts_enabled)
                    .on_toggle(|_| Message::ToggleAlertsEnabled),
            ))
            .into()
    }

    /// AIR QUALITY section: optional aqicn.org token input.
    fn render_aq_section(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        settings::section()
            .title(crate::fl!("section-air-quality"))
            .add(
                widget::Column::new()
                    .spacing(spacing.space_xxxs)
                    .push(widget::text::body(crate::fl!("settings-aqicn-token")))
                    .push(
                        widget::text_input("", &self.aqicn_token_input)
                            .on_input(Message::UpdateAqicnToken)
                            .width(cosmic::iced::Length::Fill),
                    )
                    .push(widget::text::caption(crate::fl!(
                        "settings-aqicn-token-hint"
                    ))),
            )
            .into()
    }

    /// PANEL DISPLAY section: per-element show/hide toggles for the panel button.
    fn render_panel_display_section(&self) -> Element<'_, Message> {
        settings::section()
            .title(crate::fl!("section-panel-display"))
            .add(settings::item(
                crate::fl!("show-icon"),
                widget::toggler(self.config.show_icon_in_panel)
                    .on_toggle(|_| Message::ToggleShowIconInPanel),
            ))
            .add(settings::item(
                crate::fl!("show-aqi"),
                widget::toggler(self.config.show_aqi_in_panel)
                    .on_toggle(|_| Message::ToggleShowAqiInPanel),
            ))
            .add(settings::item(
                crate::fl!("show-pressure"),
                widget::toggler(self.config.show_pressure_in_panel)
                    .on_toggle(|_| Message::ToggleShowPressureInPanel),
            ))
            .add(settings::item(
                crate::fl!("show-dew-point"),
                widget::toggler(self.config.show_dew_point_in_panel)
                    .on_toggle(|_| Message::ToggleShowDewPointInPanel),
            ))
            .add(settings::item(
                crate::fl!("show-sunrise-sunset"),
                widget::toggler(self.config.show_sunrise_sunset_in_panel)
                    .on_toggle(|_| Message::ToggleShowSunriseSunsetInPanel),
            ))
            .add(settings::item(
                crate::fl!("show-meteogram"),
                widget::toggler(self.config.show_meteogram)
                    .on_toggle(|_| Message::ToggleShowMeteogram),
            ))
            .add(settings::item(
                crate::fl!("settings-pride-accent"),
                widget::toggler(self.config.pride_accent).on_toggle(|_| Message::TogglePrideAccent),
            ))
            .into()
    }

    /// SUPPORT section: version label and a tip-jar button.
    fn render_support_section(&self) -> Element<'_, Message> {
        settings::section()
            .title(crate::fl!("settings-support"))
            .add(
                widget::Row::new()
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(widget::text::caption(format!(
                        "{} {}",
                        crate::fl!("settings-version"),
                        VERSION
                    )))
                    .push(widget::space::horizontal())
                    .push(
                        widget::button::link(crate::fl!("settings-tip-kofi"))
                            .on_press(Message::OpenKofi),
                    ),
            )
            .into()
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
/// defensively future) result, which the caller drops via `Task::none()`
/// (FIX-03 / D-08). Extracted as a pure helper for unit-testability (D-06).
fn is_current_generation(current: u64, incoming: u64) -> bool {
    current == incoming
}

#[cfg(test)]
mod tests {
    use super::*;

    // FIX-01 regression seed (D-06): byte-based truncation panicked when `max_len`
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

    // FIX-03 (D-06): the pure generation-compare helper. A result is applied only
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
