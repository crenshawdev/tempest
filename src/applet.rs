// SPDX-License-Identifier: GPL-3.0-only

use cosmic::app::{Core, Task};
use cosmic::cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry};
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::window::Id;
use cosmic::iced::{Limits, Subscription};
use cosmic::widget::{self, segmented_button, settings, text};
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
}

/// Queries cosmic-randr for the primary display resolution.
/// Returns (width, height) or None if unavailable.
fn get_screen_resolution() -> Option<(u32, u32)> {
    let list = futures::executor::block_on(cosmic_randr_shell::list()).ok()?;

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

/// Calculates popup max height based on screen resolution.
/// Uses 75% of screen height, clamped between 400-1000 pixels.
fn calculate_popup_max_height() -> f32 {
    match get_screen_resolution() {
        Some((_width, height)) => (height as f32 * 0.75).clamp(400.0, 1000.0),
        None => 650.0, // Fallback assumes ~1080p
    }
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
fn build_tab_model(active: Option<PopupTab>) -> segmented_button::SingleSelectModel {
    let mut model = segmented_button::SingleSelectModel::default();

    let tabs = [
        (PopupTab::Current, crate::fl!("tab-current")),
        (PopupTab::Hourly, crate::fl!("tab-hourly")),
        (PopupTab::Forecast, crate::fl!("tab-forecast")),
    ];

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
            tab_model: build_tab_model(tab_for_segmented_control(active_tab)),
            temperature_model: build_temperature_model(config.temperature_unit),
            measurement_model: build_measurement_model(config.measurement_system),
            pressure_model: build_pressure_model(config.pressure_unit),
            last_updated_display: None,
            military_time: false,
            showing_pollutants: false,
            showing_locations: false,
            pollen: None,
            showing_pollen: false,
            popup_max_height: calculate_popup_max_height(),
            retry_count: 0,
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
    RefreshWeather,
    WeatherUpdated(Result<WeatherData, String>),
    AirQualityUpdated(Result<AirQualityData, String>),
    AlertsUpdated(Result<Vec<Alert>, String>),
    Tick,
    ToggleAlertsEnabled,
    ToggleAutoUnits,
    ToggleShowAqiInPanel,
    ToggleShowIconInPanel,
    ToggleShowPressureInPanel,
    ToggleShowDewPointInPanel,
    ToggleShowSunriseSunsetInPanel,
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
    PollenUpdated(Result<Option<PollenData>, String>),
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
        let active_tab = config.default_tab;
        let tab_model = build_tab_model(tab_for_segmented_control(active_tab));
        let temperature_model = build_temperature_model(config.temperature_unit);
        let measurement_model = build_measurement_model(config.measurement_system);

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
            military_time,
            ..Default::default()
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

        (app, task)
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

    fn view(&self) -> Element<'_, Self::Message> {
        use chrono::{Local, Timelike};
        use cosmic::iced::Alignment;

        let spacing = cosmic::theme::spacing();

        // Determine if it's night time using actual sunrise/sunset data
        let is_night = self
            .weather_data
            .as_ref()
            .and_then(|w| w.forecast.first())
            .map(|day| is_night_time(&day.sunrise, &day.sunset))
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

        let icon = widget::icon::from_name(icon_name).size(16).symbolic(true);

        let temperature_text = text(&self.display_label);

        let has_alerts = !self.alerts.is_empty();
        let alert_icon = widget::icon::from_name("dialog-warning-symbolic")
            .size(16)
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

        let data = if self.core.applet.is_horizontal() {
            let mut row = widget::Row::new()
                .align_y(Alignment::Center)
                .spacing(spacing.space_xxs);
            if has_alerts {
                row = row.push(alert_icon);
            }
            if self.config.show_icon_in_panel {
                row = row.push(icon);
            }
            row = row.push(temperature_text);
            for label in [&aqi_label, &dew_point_label, &pressure_label, &sun_label]
                .into_iter()
                .flatten()
            {
                row = row.push(widget::text::caption("|"));
                row = row.push(widget::text::caption(label.clone()));
            }
            Element::from(row)
        } else {
            let mut col = widget::Column::new()
                .align_x(Alignment::Center)
                .spacing(spacing.space_xxs);
            if has_alerts {
                col = col.push(alert_icon);
            }
            if self.config.show_icon_in_panel {
                col = col.push(icon);
            }
            col = col.push(temperature_text);
            for label in [&aqi_label, &dew_point_label, &pressure_label, &sun_label]
                .into_iter()
                .flatten()
            {
                col = col.push(widget::text::caption(label.clone()));
            }
            Element::from(col)
        };

        let button = widget::button::custom(data)
            .class(cosmic::theme::Button::AppletIcon)
            .on_press(Message::TogglePopup);

        widget::autosize::autosize(button, widget::Id::unique()).into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        let spacing = cosmic::theme::spacing();
        let mut column = widget::Column::new().spacing(spacing.space_xs).padding([
            spacing.space_xs,
            spacing.space_xs,
            spacing.space_m,
            spacing.space_xs,
        ]);

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

        // Alert button - styled to stand out when alerts are active
        let alerts_btn = widget::button::icon(widget::icon::from_name(alerts_icon))
            .on_press(Message::SelectTab(PopupTab::Alerts))
            .padding(spacing.space_xs);
        let alerts_btn = if has_alerts {
            alerts_btn.class(cosmic::theme::Button::Destructive)
        } else {
            alerts_btn
        };

        header = header
            .push(widget::space::horizontal())
            .push(
                widget::button::icon(widget::icon::from_name("view-refresh-symbolic"))
                    .on_press(Message::RefreshWeather)
                    .padding(spacing.space_xs),
            )
            .push(alerts_btn)
            .push(
                widget::button::icon(widget::icon::from_name("emblem-system-symbolic"))
                    .on_press(Message::SelectTab(PopupTab::Settings))
                    .padding(spacing.space_xs),
            );

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
                            .push(widget::icon::from_name("go-next-symbolic").size(14)),
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
                        .push(widget::icon::from_name("dialog-error-symbolic").size(40))
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
                        .push(widget::icon::from_name("content-loading-symbolic").size(40))
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
                PopupTab::Settings => column = column.push(self.render_settings_tab()),
            }
        }

        let scrollable = widget::scrollable(column).height(cosmic::iced::Length::Shrink);

        self.core
            .applet
            .popup_container(scrollable)
            .limits(self.popup_limits())
            .into()
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::TogglePopup => {
                return if let Some(p) = self.popup.take() {
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
                    get_popup(popup_settings)
                }
            }
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                    self.showing_pollutants = false;
                }
            }
            Message::RefreshWeather => {
                self.is_loading = true;
                self.error_message = None;

                let lat = self.config.latitude;
                let lon = self.config.longitude;
                let temp_unit = self.config.temperature_unit;
                let measurement = self.config.measurement_system;
                let alerts_enabled = self.config.alerts_enabled;

                // Fetch weather and air quality in parallel
                let weather_task = Task::perform(
                    async move {
                        fetch_weather(lat, lon, temp_unit, measurement)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    |result| Action::App(Message::WeatherUpdated(result)),
                );

                let aqicn_token = self.config.aqicn_token.clone();
                let air_quality_task = Task::perform(
                    async move {
                        fetch_air_quality(lat, lon, aqicn_token.as_deref())
                            .await
                            .map_err(|e| e.to_string())
                    },
                    |result| Action::App(Message::AirQualityUpdated(result)),
                );

                // Fetch alerts if enabled
                let alerts_task = if alerts_enabled {
                    Task::perform(
                        async move { fetch_alerts(lat, lon).await.map_err(|e| e.to_string()) },
                        |result| Action::App(Message::AlertsUpdated(result)),
                    )
                } else {
                    Task::none()
                };

                // Pollen is region-optional. fetch_pollen returns Ok(None) for
                // coordinates outside CAMS coverage, so call unconditionally
                // and let the render layer decide whether to surface the row.
                let pollen_task = Task::perform(
                    async move { fetch_pollen(lat, lon).await.map_err(|e| e.to_string()) },
                    |result| Action::App(Message::PollenUpdated(result)),
                );

                return Task::batch([weather_task, air_quality_task, alerts_task, pollen_task]);
            }
            Message::WeatherUpdated(result) => {
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
                        self.error_message = None;

                        // Update last updated timestamp and cache formatted display
                        let now = chrono::Local::now();
                        self.config.last_updated = Some(now.timestamp());
                        let fmt = if self.military_time {
                            "%H:%M"
                        } else {
                            "%I:%M %p"
                        };
                        let formatted = now.format(fmt).to_string();
                        self.last_updated_display = Some(if self.military_time {
                            formatted
                        } else {
                            formatted.trim_start_matches('0').to_string()
                        });
                        self.save_config();
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch weather: {}", e);
                        self.display_label = "ERR".to_string();
                        self.current_condition = weathervane::WeatherCondition::Unknown;
                        self.error_message = Some(crate::fl!("weather-fetch-error"));

                        // Schedule a retry with exponential backoff
                        const BACKOFF_SECS: [u64; 4] = [5, 15, 30, 60];
                        if (self.retry_count as usize) < BACKOFF_SECS.len() {
                            let delay = BACKOFF_SECS[self.retry_count as usize];
                            self.retry_count += 1;
                            tracing::info!("Scheduling retry {} in {}s", self.retry_count, delay);
                            return Task::perform(
                                async move {
                                    tokio::time::sleep(Duration::from_secs(delay)).await;
                                    Message::RetryFetch
                                },
                                Action::App,
                            );
                        }
                        tracing::warn!(
                            "Giving up after {} retries, waiting for next refresh",
                            self.retry_count
                        );
                    }
                }
            }
            Message::AirQualityUpdated(result) => match result {
                Ok(data) => {
                    self.current_aqi = Some((data.aqi, data.standard));
                    self.air_quality = Some(data);
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch air quality: {}", e);
                    self.current_aqi = None;
                    self.air_quality = None;
                }
            },
            Message::AlertsUpdated(result) => match result {
                Ok(new_alerts) => {
                    // Send notifications for new alerts
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
            },
            Message::Tick => {
                self.retry_count = 0;
                return Task::perform(async { Message::RefreshWeather }, Action::App);
            }
            Message::ToggleAlertsEnabled => {
                self.config.alerts_enabled = !self.config.alerts_enabled;
                if !self.config.alerts_enabled {
                    self.alerts.clear();
                }
                self.save_config();
                return Task::perform(async { Message::RefreshWeather }, Action::App);
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
                return Task::perform(async { Message::RefreshWeather }, Action::App);
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
            Message::SelectLocation(idx) => {
                if let Some(location) = self.search_results.get(idx) {
                    let country = location.country.clone();
                    self.config.latitude = location.latitude;
                    self.config.longitude = location.longitude;
                    self.config.location_name = location.display_name.clone();
                    self.config.use_auto_location = false;
                    // Update manual location storage
                    self.config.manual_latitude = Some(location.latitude);
                    self.config.manual_longitude = Some(location.longitude);
                    self.config.manual_location_name = Some(location.display_name.clone());

                    self.apply_units_for_country(&country);

                    self.city_input.clear();
                    self.search_results.clear();
                    self.save_config();
                    return Task::perform(async { Message::RefreshWeather }, Action::App);
                }
            }
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
            Message::ToggleAutoLocation => {
                self.config.use_auto_location = !self.config.use_auto_location;

                if self.config.use_auto_location {
                    // Save current manual location before switching to auto
                    self.config.manual_latitude = Some(self.config.latitude);
                    self.config.manual_longitude = Some(self.config.longitude);
                    self.config.manual_location_name = Some(self.config.location_name.clone());
                    self.save_config();

                    return Task::perform(
                        async { detect_location().await.map_err(|e| e.to_string()) },
                        |result| Action::App(Message::LocationDetected(result)),
                    );
                } else {
                    // Restore previous manual location if available
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

                    return Task::perform(async { Message::RefreshWeather }, Action::App);
                }
            }
            Message::DetectLocation => {
                return Task::perform(
                    async { detect_location().await.map_err(|e| e.to_string()) },
                    |result| Action::App(Message::LocationDetected(result)),
                );
            }
            Message::LocationDetected(result) => match result {
                Ok(loc) => {
                    self.config.latitude = loc.latitude;
                    self.config.longitude = loc.longitude;
                    self.config.location_name = loc.display_name;
                    let country = loc.country;

                    self.apply_units_for_country(&country);

                    self.save_config();
                    return Task::perform(async { Message::RefreshWeather }, Action::App);
                }
                Err(e) => {
                    tracing::error!("Failed to detect location: {}", e);
                }
            },
            Message::SelectTab(tab) => {
                self.active_tab = tab;
                self.config.default_tab = tab;
                self.showing_pollutants = false;
                // Rebuild the model to sync selection state
                self.tab_model = build_tab_model(tab_for_segmented_control(tab));
                self.save_config();
            }
            Message::TabActivated(entity) => {
                self.tab_model.activate(entity);
                if let Some(&tab) = self.tab_model.data::<PopupTab>(entity) {
                    self.active_tab = tab;
                    self.config.default_tab = tab;
                    self.showing_pollutants = false;
                    self.save_config();
                }
            }
            Message::TemperatureUnitActivated(entity) => {
                self.temperature_model.activate(entity);
                if let Some(&unit) = self.temperature_model.data::<TemperatureUnit>(entity) {
                    self.config.temperature_unit = unit;
                    self.save_config();
                    return Task::perform(async { Message::RefreshWeather }, Action::App);
                }
            }
            Message::MeasurementActivated(entity) => {
                self.measurement_model.activate(entity);
                if let Some(&system) = self.measurement_model.data::<MeasurementSystem>(entity) {
                    self.config.measurement_system = system;
                    self.save_config();
                    return Task::perform(async { Message::RefreshWeather }, Action::App);
                }
            }
            Message::PressureUnitActivated(entity) => {
                self.pressure_model.activate(entity);
                if let Some(&unit) = self.pressure_model.data::<PressureUnit>(entity) {
                    self.config.pressure_unit = unit;
                    self.save_config();
                }
            }
            Message::SystemTimeConfig(config) => {
                self.military_time = config.military_time;
                // Refresh the cached timestamp display with new format
                if let Some(timestamp) = self.config.last_updated {
                    if let Some(dt) = chrono::DateTime::from_timestamp(timestamp, 0) {
                        let local = dt.with_timezone(&chrono::Local);
                        let fmt = if self.military_time {
                            "%H:%M"
                        } else {
                            "%I:%M %p"
                        };
                        let formatted = local.format(fmt).to_string();
                        self.last_updated_display = Some(if self.military_time {
                            formatted
                        } else {
                            formatted.trim_start_matches('0').to_string()
                        });
                    }
                }
            }
            Message::ShowPollutants => {
                self.showing_pollutants = true;
            }
            Message::HidePollutants => {
                self.showing_pollutants = false;
            }
            Message::PollenUpdated(result) => match result {
                Ok(data) => self.pollen = Some(data),
                Err(e) => {
                    // Pollen is region-optional. Treat network failures as
                    // "no data" rather than surfacing them — the alternative
                    // is a blip of "Pollen unavailable" UI whenever the
                    // network hiccups, which is worse than no UI at all.
                    self.pollen = Some(None);
                    tracing::warn!("pollen fetch failed: {e}");
                }
            },
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
                    return Task::perform(async { Message::RefreshWeather }, Action::App);
                }
            }
            Message::SaveLocation(idx) => {
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
                return Task::perform(async { Message::RefreshWeather }, Action::App);
            }
            Message::NetworkChanged(crate::network::NetworkEvent::Connected) => {
                self.retry_count = 0;
                return Task::perform(async { Message::RefreshWeather }, Action::App);
            }
            Message::SystemResumed => {
                weathervane::reset_http_client();
                self.retry_count = 0;
                return Task::perform(async { Message::RefreshWeather }, Action::App);
            }
        }
        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

impl Tempest {
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
        let mut col = widget::Column::new().spacing(spacing.space_s).padding([
            0,
            spacing.space_m,
            0,
            spacing.space_m,
        ]);

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
            let aqi_row = widget::button::custom(
                widget::Row::new()
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
                    .push(widget::icon::from_name("go-next-symbolic").size(16)),
            )
            .class(cosmic::theme::Button::Text)
            .on_press(Message::ShowPollutants);

            col = col.push(aqi_row);

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
                col = col.push(
                    widget::text::caption(crate::fl!("aqicn-attribution"))
                        .class(cosmic::theme::Text::Accent),
                );
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

                let pollen_row = widget::button::custom(
                    widget::Row::new()
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(
                            widget::Column::new()
                                .push(widget::text::title4(headline))
                                .push(widget::text::caption(caption)),
                        )
                        .push(widget::space::horizontal())
                        .push(widget::icon::from_name("go-next-symbolic").size(16)),
                )
                .class(cosmic::theme::Button::Text)
                .on_press(Message::ShowPollen);

                col = col.push(pollen_row);
            }
        }

        col.into()
    }

    /// Renders the pollutants sub-view with Back button and pollutant list.
    fn render_pollutants_view(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let mut col = widget::Column::new().spacing(spacing.space_m).padding([
            0,
            spacing.space_m,
            0,
            spacing.space_m,
        ]);

        let close_btn = widget::button::custom(
            widget::Row::new()
                .spacing(spacing.space_xxxs)
                .align_y(cosmic::iced::Alignment::Center)
                .push(widget::text::body(crate::fl!("air-quality-close")))
                .push(widget::icon::from_name("go-next-symbolic").size(16)),
        )
        .class(cosmic::theme::Button::Link)
        .on_press(Message::HidePollutants);

        let header = widget::Row::new()
            .align_y(cosmic::iced::Alignment::Center)
            .push(
                widget::container(widget::text::heading(crate::fl!("air-quality-index")))
                    .width(cosmic::iced::Length::Fill)
                    .align_x(cosmic::iced::alignment::Horizontal::Center),
            )
            .push(close_btn);

        col = col.push(header);

        // Pollutant list
        if let Some(ref aq) = self.air_quality {
            let list = widget::list_column()
                .add(Self::pollutant_row(
                    crate::fl!("label-co"),
                    format!("{:.1} ug/m3", aq.carbon_monoxide),
                ))
                .add(Self::pollutant_row(
                    crate::fl!("label-no2"),
                    format!("{:.1} ug/m3", aq.nitrogen_dioxide),
                ))
                .add(Self::pollutant_row(
                    crate::fl!("label-ozone"),
                    format!("{:.1} ug/m3", aq.ozone),
                ))
                .add(Self::pollutant_row(
                    crate::fl!("label-pm10"),
                    format!("{:.1} ug/m3", aq.pm10),
                ))
                .add(Self::pollutant_row(
                    crate::fl!("label-pm25"),
                    format!("{:.1} ug/m3", aq.pm2_5),
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
        let mut col = widget::Column::new().spacing(spacing.space_m).padding([
            0,
            spacing.space_m,
            0,
            spacing.space_m,
        ]);

        let close_btn = widget::button::custom(
            widget::Row::new()
                .spacing(spacing.space_xxxs)
                .align_y(cosmic::iced::Alignment::Center)
                .push(widget::text::body(crate::fl!("air-quality-close")))
                .push(widget::icon::from_name("go-next-symbolic").size(16)),
        )
        .class(cosmic::theme::Button::Link)
        .on_press(Message::HidePollen);

        let header = widget::Row::new()
            .align_y(cosmic::iced::Alignment::Center)
            .push(
                widget::container(widget::text::heading(crate::fl!("label-pollen")))
                    .width(cosmic::iced::Length::Fill)
                    .align_x(cosmic::iced::alignment::Horizontal::Center),
            )
            .push(close_btn);

        col = col.push(header);

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

            col = col.push(
                widget::text::caption(crate::fl!("pollen-attribution"))
                    .class(cosmic::theme::Text::Accent),
            );
        }

        col.into()
    }

    /// Renders the saved locations sub-view with back button and location list.
    fn render_locations_view(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let mut col = widget::Column::new().spacing(spacing.space_xxs).padding([
            0,
            spacing.space_xxs,
            0,
            spacing.space_m,
        ]);

        // Back button
        let back_btn = widget::button::custom(
            widget::Row::new()
                .spacing(spacing.space_xxxs)
                .align_y(cosmic::iced::Alignment::Center)
                .push(widget::icon::from_name("go-previous-symbolic").size(16))
                .push(widget::text::body(crate::fl!("locations-back"))),
        )
        .class(cosmic::theme::Button::Link)
        .on_press(Message::HideLocations);

        col = col.push(back_btn);
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
        let mut col = widget::Column::new().spacing(spacing.space_xxs).padding([
            0,
            spacing.space_m,
            0,
            spacing.space_m,
        ]);

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
                                .size(40)
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

                list = list.add(
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
                                    widget::scrollable(widget::text::caption(&alert.description))
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
                            .size(20)
                            .symbolic(true),
                    )
                    .push(widget::text::body(
                        self.config.temperature_unit.format(hour.temperature),
                    ))
                    .push(widget::text::caption(format!(
                        "{}%",
                        hour.precipitation_probability
                    )));

                row = row.push(
                    widget::container(cell)
                        .width(cosmic::iced::Length::FillPortion(1))
                        .align_x(cosmic::iced::alignment::Horizontal::Center),
                );
            }

            // Pad incomplete rows
            for _ in chunk.len()..hours_per_row {
                row = row.push(
                    widget::container(widget::Space::new())
                        .width(cosmic::iced::Length::FillPortion(1)),
                );
            }

            col = col.push(row);
        }

        col.into()
    }

    /// Renders the 7-day Forecast tab content.
    fn render_forecast_tab(&self, weather: &WeatherData) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let mut col = widget::Column::new()
            .spacing(spacing.space_xxs)
            .padding([0, spacing.space_xxs, 0, spacing.space_m])
            .width(cosmic::iced::Length::Fill);

        // Table header
        col = col.push(
            widget::Row::new()
                .spacing(spacing.space_m)
                .push(
                    widget::container(widget::text::caption(crate::fl!("forecast-day")))
                        .width(cosmic::iced::Length::FillPortion(3)),
                )
                .push(widget::Space::new().width(spacing.space_m))
                .push(
                    widget::container(widget::text::caption(crate::fl!("forecast-high")))
                        .width(cosmic::iced::Length::FillPortion(1)),
                )
                .push(
                    widget::container(widget::text::caption(crate::fl!("forecast-low")))
                        .width(cosmic::iced::Length::FillPortion(1)),
                )
                .push(
                    widget::container(widget::text::caption(crate::fl!("forecast-conditions")))
                        .width(cosmic::iced::Length::FillPortion(2)),
                ),
        );
        col = col.push(widget::divider::horizontal::default());

        // Data rows
        for day in &weather.forecast {
            col = col.push(
                widget::Row::new()
                    .spacing(spacing.space_m)
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(
                        widget::container(widget::text::body(format_date(&day.date)))
                            .width(cosmic::iced::Length::FillPortion(3)),
                    )
                    .push(
                        widget::icon::from_name(day.condition.icon_name(false))
                            .size(20)
                            .symbolic(true),
                    )
                    .push(
                        widget::container(widget::text::body(
                            self.config.temperature_unit.format(day.temp_max),
                        ))
                        .width(cosmic::iced::Length::FillPortion(1)),
                    )
                    .push(
                        widget::container(widget::text::body(
                            self.config.temperature_unit.format(day.temp_min),
                        ))
                        .width(cosmic::iced::Length::FillPortion(1)),
                    )
                    .push(
                        widget::container(
                            widget::text::body(condition_to_description(day.condition))
                                .wrapping(cosmic::iced::widget::text::Wrapping::None)
                                .ellipsize(cosmic::iced::widget::text::Ellipsize::End(
                                    cosmic::iced::core::text::EllipsizeHeightLimit::Lines(1),
                                )),
                        )
                        .width(cosmic::iced::Length::FillPortion(2)),
                    ),
            );
        }

        col.into()
    }

    /// Creates a styled section header for the settings tab.
    fn section_header(label: String) -> Element<'static, Message> {
        widget::text::caption(label)
            .class(cosmic::theme::Text::Accent)
            .into()
    }

    /// Renders the Settings tab content.
    fn render_settings_tab(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let mut col = widget::Column::new().spacing(spacing.space_xs).padding([
            0,
            spacing.space_xxs,
            0,
            spacing.space_m,
        ]);

        // LOCATION section
        col = col.push(Self::section_header(crate::fl!("section-location")));

        col = col.push(settings::item(
            crate::fl!("settings-auto-detect"),
            widget::toggler(self.config.use_auto_location)
                .on_toggle(|_| Message::ToggleAutoLocation),
        ));

        if self.config.use_auto_location {
            // Auto-detect enabled: show location with subtitle and refresh button
            col = col.push(
                widget::Row::new()
                    .spacing(spacing.space_xxs)
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(
                        widget::Column::new()
                            .push(widget::text::body(&self.config.location_name))
                            .push(
                                widget::text::caption(crate::fl!("detected-via-ip"))
                                    .class(cosmic::theme::Text::Accent),
                            )
                            .width(cosmic::iced::Length::Fill),
                    )
                    .push(
                        widget::button::standard(crate::fl!("settings-refresh"))
                            .on_press(Message::DetectLocation),
                    ),
            );
        } else {
            // Manual mode: show search input
            col = col.push(
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

            // Search results with select and save buttons
            for (idx, result) in self.search_results.iter().enumerate() {
                col = col.push(
                    widget::Row::new()
                        .spacing(spacing.space_xxxs)
                        .align_y(cosmic::iced::Alignment::Center)
                        .push(
                            widget::button::text(&result.display_name)
                                .on_press(Message::SelectLocation(idx))
                                .padding(spacing.space_xxs)
                                .width(cosmic::iced::Length::Fill),
                        )
                        .push(
                            widget::button::icon(
                                widget::icon::from_name("bookmark-new-symbolic").size(16),
                            )
                            .on_press(Message::SaveLocation(idx))
                            .padding(spacing.space_xxs),
                        ),
                );
            }

            // Show current location with "Manually selected" subtitle
            col = col.push(
                widget::Column::new()
                    .push(widget::text::body(&self.config.location_name))
                    .push(
                        widget::text::caption(crate::fl!("manually-selected"))
                            .class(cosmic::theme::Text::Accent),
                    ),
            );
        }

        // SAVED LOCATIONS section
        if !self.config.saved_locations.is_empty() {
            col = col.push(widget::divider::horizontal::default());
            col = col.push(Self::section_header(crate::fl!("section-saved-locations")));

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

                row = row.push(
                    widget::button::icon(widget::icon::from_name("edit-delete-symbolic").size(16))
                        .on_press(Message::RemoveSavedLocation(idx))
                        .padding(spacing.space_xxxs),
                );

                list = list.add(row);
            }
            col = col.push(list);
        }

        // UNITS section
        col = col.push(widget::divider::horizontal::default());
        col = col.push(Self::section_header(crate::fl!("section-units")));

        col = col.push(settings::item(
            crate::fl!("settings-temperature"),
            widget::segmented_control::horizontal(&self.temperature_model)
                .on_activate(Message::TemperatureUnitActivated),
        ));

        col = col.push(settings::item(
            crate::fl!("settings-measurement"),
            widget::segmented_control::horizontal(&self.measurement_model)
                .on_activate(Message::MeasurementActivated),
        ));

        col = col.push(settings::item(
            crate::fl!("settings-pressure"),
            widget::segmented_control::horizontal(&self.pressure_model)
                .on_activate(Message::PressureUnitActivated),
        ));

        col = col.push(settings::item(
            crate::fl!("settings-auto-units"),
            widget::Row::new()
                .spacing(spacing.space_xxs)
                .align_y(cosmic::iced::Alignment::Center)
                .push(widget::text::caption(crate::fl!(
                    "settings-auto-units-hint"
                )))
                .push(
                    widget::toggler(self.config.auto_units).on_toggle(|_| Message::ToggleAutoUnits),
                ),
        ));

        // UPDATES section
        col = col.push(widget::divider::horizontal::default());
        col = col.push(Self::section_header(crate::fl!("section-updates")));

        col = col.push(settings::item(
            crate::fl!("settings-refresh-interval"),
            widget::Row::new()
                .spacing(spacing.space_xxs)
                .align_y(cosmic::iced::Alignment::Center)
                .push(widget::text::body(crate::fl!("settings-min")))
                .push(
                    widget::text_input("15", &self.refresh_input)
                        .on_input(Message::UpdateRefreshInterval),
                ),
        ));

        col = col.push(settings::item(
            crate::fl!("settings-weather-alerts"),
            widget::toggler(self.config.alerts_enabled).on_toggle(|_| Message::ToggleAlertsEnabled),
        ));

        // AIR QUALITY section
        col = col.push(widget::divider::horizontal::default());
        col = col.push(Self::section_header(crate::fl!("section-air-quality")));

        col = col.push(
            widget::Column::new()
                .spacing(spacing.space_xxxs)
                .push(widget::text::body(crate::fl!("settings-aqicn-token")))
                .push(
                    widget::text_input("", &self.aqicn_token_input)
                        .on_input(Message::UpdateAqicnToken)
                        .width(cosmic::iced::Length::Fill),
                )
                .push(
                    widget::text::caption(crate::fl!("settings-aqicn-token-hint"))
                        .class(cosmic::theme::Text::Accent),
                ),
        );

        // PANEL DISPLAY section
        col = col.push(widget::divider::horizontal::default());
        col = col.push(Self::section_header(crate::fl!("section-panel-display")));

        col = col.push(settings::item(
            crate::fl!("show-icon"),
            widget::toggler(self.config.show_icon_in_panel)
                .on_toggle(|_| Message::ToggleShowIconInPanel),
        ));

        col = col.push(settings::item(
            crate::fl!("show-aqi"),
            widget::toggler(self.config.show_aqi_in_panel)
                .on_toggle(|_| Message::ToggleShowAqiInPanel),
        ));

        col = col.push(settings::item(
            crate::fl!("show-pressure"),
            widget::toggler(self.config.show_pressure_in_panel)
                .on_toggle(|_| Message::ToggleShowPressureInPanel),
        ));

        col = col.push(settings::item(
            crate::fl!("show-dew-point"),
            widget::toggler(self.config.show_dew_point_in_panel)
                .on_toggle(|_| Message::ToggleShowDewPointInPanel),
        ));

        col = col.push(settings::item(
            crate::fl!("show-sunrise-sunset"),
            widget::toggler(self.config.show_sunrise_sunset_in_panel)
                .on_toggle(|_| Message::ToggleShowSunriseSunsetInPanel),
        ));

        // SUPPORT section
        col = col.push(widget::divider::horizontal::default());
        col = col.push(Self::section_header(crate::fl!("settings-support")));

        col = col.push(
            widget::Row::new()
                .align_y(cosmic::iced::Alignment::Center)
                .push(
                    widget::text::caption(format!(
                        "{} {}",
                        crate::fl!("settings-version"),
                        VERSION
                    ))
                    .class(cosmic::theme::Text::Accent),
                )
                .push(widget::space::horizontal())
                .push(
                    widget::button::standard(crate::fl!("settings-tip-kofi"))
                        .on_press(Message::OpenKofi),
                ),
        );

        col.into()
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

    if output.len() > max_len {
        output.truncate(max_len);
        output.push_str("...");
    }

    output
}
