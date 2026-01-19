// SPDX-License-Identifier: GPL-3.0-only

use cosmic::app::{Core, Task};
use cosmic::cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry};
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::window::Id;
use cosmic::iced::{Limits, Subscription};
use cosmic::iced_futures::Subscription as IcedSubscription;
use cosmic::widget::{self, segmented_button, settings, text};
use cosmic::{Action, Application, Element};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;

use crate::config::{Config, MeasurementSystem, PopupTab, TemperatureUnit};
use crate::weather::{
    aqi_to_description, detect_location, fetch_air_quality, fetch_alerts, fetch_weather,
    format_date, format_hour, format_time, is_night_time, search_city, uses_imperial_units,
    weathercode_to_description, weathercode_to_icon_name, wind_direction_to_compass,
    AirQualityData, Alert, AlertSeverity, AqiStandard, LocationResult, WeatherData,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// System-wide time format preference from COSMIC time applet.
#[derive(Debug, Clone, Default, PartialEq, Eq, CosmicConfigEntry, Deserialize, Serialize)]
#[version = 1]
pub struct TimeAppletConfig {
    #[serde(default)]
    pub military_time: bool,
}

/// This is the struct that represents your application.
/// It is used to define the data that will be used by your application.
pub struct Tempest {
    /// Application state which is managed by the COSMIC runtime.
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
    /// Search results
    search_results: Vec<LocationResult>,
    /// Display label for panel button
    display_label: String,
    /// Current weather code for icon display
    current_weathercode: i32,
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
    /// Cached formatted timestamp for display (avoids recomputing on every render)
    last_updated_display: Option<String>,
    /// 24-hour time format when true, 12-hour with AM/PM when false.
    military_time: bool,
    /// Whether the pollutants sub-view is currently displayed.
    showing_pollutants: bool,
    /// Cached max popup height based on screen resolution.
    popup_max_height: f32,
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
/// Uses 70% of screen height, clamped between 400-900 pixels.
fn calculate_popup_max_height() -> f32 {
    match get_screen_resolution() {
        Some((_width, height)) => (height as f32 * 0.7).clamp(400.0, 900.0),
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
            search_results: Vec::new(),
            display_label: "...".to_string(),
            current_weathercode: 0,
            current_aqi: None,
            is_loading: true,
            error_message: None,
            active_tab,
            tab_model: build_tab_model(tab_for_segmented_control(active_tab)),
            temperature_model: build_temperature_model(config.temperature_unit),
            measurement_model: build_measurement_model(config.measurement_system),
            last_updated_display: None,
            military_time: false,
            showing_pollutants: false,
            popup_max_height: calculate_popup_max_height(),
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
    DetectLocation,
    LocationDetected(Result<(f64, f64, String, String), String>),
    ToggleAutoLocation,
    SelectTab(PopupTab),
    TabActivated(segmented_button::Entity),
    TemperatureUnitActivated(segmented_button::Entity),
    MeasurementActivated(segmented_button::Entity),
    SystemTimeConfig(TimeAppletConfig),
    ShowPollutants,
    HidePollutants,
    OpenKofi,
}

/// Implement the `Application` trait for your application.
/// This is where you define the behavior of your application.
///
/// The `Application` trait requires you to define the following types and constants:
/// - `Executor` is the async executor that will be used to run your application's commands.
/// - `Flags` is the data that your application needs to use before it starts.
/// - `Message` is the enum that contains all the possible variants that your application will need to transmit messages.
/// - `APP_ID` is the unique identifier of your application.
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

    /// This is the entry point of your application, it is where you initialize your application.
    ///
    /// Any work that needs to be done before the application starts should be done here.
    ///
    /// - `core` is used to passed on for you by libcosmic to use in the core of your own application.
    /// - `flags` is used to pass in any data that your application needs to use before it starts.
    /// - `Task` type is used to send messages to your application. `Task::none()` can be used to send no messages to your application.
    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let config_handler = cosmic::cosmic_config::Config::new(Self::APP_ID, Config::VERSION).ok();
        let config = config_handler
            .as_ref()
            .and_then(|h| Config::get_entry(h).ok())
            .unwrap_or_default();

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
        let tick = IcedSubscription::run_with_id(
            (std::any::TypeId::of::<Self>(), interval_minutes),
            async_stream::stream! {
                let interval = Duration::from_secs(interval_minutes * 60);
                loop {
                    tokio::time::sleep(interval).await;
                    yield Message::Tick;
                }
            },
        );

        // Watch system time config for 12/24 hour format changes
        let time_config = self
            .core
            .watch_config::<TimeAppletConfig>("com.system76.CosmicAppletTime")
            .map(|update| Message::SystemTimeConfig(update.config));

        Subscription::batch([tick, time_config])
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    /// This is the main view of your application, it is the root of your widget tree.
    ///
    /// The `Element` type is used to represent the visual elements of your application,
    /// it has a `Message` associated with it, which dictates what type of message it can send.
    ///
    /// To get a better sense of which widgets are available, check out the `widget` module.
    fn view(&self) -> Element<'_, Self::Message> {
        use chrono::{Local, Timelike};
        use cosmic::iced::Alignment;

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
            weathercode_to_icon_name(self.current_weathercode, is_night)
        };

        let icon = widget::icon::from_name(icon_name).size(16).symbolic(true);

        let temperature_text = text(&self.display_label);

        let has_alerts = !self.alerts.is_empty();
        let alert_icon = widget::icon::from_name("dialog-warning-symbolic")
            .size(18)
            .symbolic(true);

        let data = if self.core.applet.is_horizontal() {
            let mut row = widget::row().align_y(Alignment::Center).spacing(4);
            if has_alerts {
                row = row.push(alert_icon);
            }
            if self.config.show_icon_in_panel {
                row = row.push(icon);
            }
            row = row.push(temperature_text);
            if self.config.show_aqi_in_panel {
                if let Some((aqi, _)) = self.current_aqi {
                    row = row.push(text("|").size(12));
                    row = row.push(text(crate::fl!("aqi-label", value = aqi)));
                }
            }
            if let Some(weather) = &self.weather_data {
                if self.config.show_dew_point_in_panel {
                    let dew_point_str = self
                        .config
                        .temperature_unit
                        .format(weather.current.dew_point);
                    row = row.push(text("|").size(12));
                    row = row.push(text(crate::fl!(
                        "panel-dew-point",
                        value = dew_point_str.as_str()
                    )));
                }
                if self.config.show_pressure_in_panel {
                    let pressure_str = format!("{:.0}", weather.current.pressure);
                    row = row.push(text("|").size(12));
                    row = row.push(text(crate::fl!(
                        "panel-pressure",
                        value = pressure_str.as_str()
                    )));
                }
                if self.config.show_sunrise_sunset_in_panel {
                    if let Some(first_day) = weather.forecast.first() {
                        let sunrise = format_time(&first_day.sunrise, self.military_time);
                        let sunset = format_time(&first_day.sunset, self.military_time);
                        row = row.push(text("|").size(12));
                        row = row.push(text(format!("{}/{}", sunrise, sunset)));
                    }
                }
            }
            Element::from(row)
        } else {
            let mut col = widget::column().align_x(Alignment::Center).spacing(4);
            if has_alerts {
                col = col.push(alert_icon);
            }
            if self.config.show_icon_in_panel {
                col = col.push(icon);
            }
            col = col.push(temperature_text);
            if self.config.show_aqi_in_panel {
                if let Some((aqi, _)) = self.current_aqi {
                    col = col.push(text(crate::fl!("aqi-label", value = aqi)).size(12));
                }
            }
            if let Some(weather) = &self.weather_data {
                if self.config.show_dew_point_in_panel {
                    let dew_point_str = self
                        .config
                        .temperature_unit
                        .format(weather.current.dew_point);
                    col = col.push(
                        text(crate::fl!(
                            "panel-dew-point",
                            value = dew_point_str.as_str()
                        ))
                        .size(12),
                    );
                }
                if self.config.show_pressure_in_panel {
                    let pressure_str = format!("{:.0}", weather.current.pressure);
                    col = col.push(
                        text(crate::fl!("panel-pressure", value = pressure_str.as_str())).size(12),
                    );
                }
                if self.config.show_sunrise_sunset_in_panel {
                    if let Some(first_day) = weather.forecast.first() {
                        let sunrise = format_time(&first_day.sunrise, self.military_time);
                        let sunset = format_time(&first_day.sunset, self.military_time);
                        col = col.push(text(format!("{}/{}", sunrise, sunset)).size(12));
                    }
                }
            }
            Element::from(col)
        };

        let button = widget::button::custom(data)
            .class(cosmic::theme::Button::AppletIcon)
            .on_press(Message::TogglePopup);

        widget::autosize::autosize(button, widget::Id::unique()).into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        let mut column = widget::column()
            .spacing(10)
            .padding([10, 10, 20, 10])
            .width(cosmic::iced::Length::Fixed(420.0));

        // Header row with timestamp and action buttons
        let has_alerts = !self.alerts.is_empty();
        let alerts_icon = if has_alerts {
            "dialog-warning-symbolic"
        } else {
            "weather-clear-symbolic"
        };

        let mut header = widget::row()
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center);

        // Add timestamp if available
        if let Some(ref formatted_time) = self.last_updated_display {
            let l_updated = crate::fl!("updated", time = formatted_time.as_str());
            header = header.push(text(l_updated).size(12));
        }

        // Alert button - styled to stand out when alerts are active
        let alerts_btn = widget::button::icon(widget::icon::from_name(alerts_icon))
            .on_press(Message::SelectTab(PopupTab::Alerts))
            .padding(6);
        let alerts_btn = if has_alerts {
            alerts_btn.class(cosmic::theme::Button::Destructive)
        } else {
            alerts_btn
        };

        header = header
            .push(widget::horizontal_space())
            .push(
                widget::button::icon(widget::icon::from_name("view-refresh-symbolic"))
                    .on_press(Message::RefreshWeather)
                    .padding(6),
            )
            .push(alerts_btn)
            .push(
                widget::button::icon(widget::icon::from_name("emblem-system-symbolic"))
                    .on_press(Message::SelectTab(PopupTab::Settings))
                    .padding(6),
            );

        column = column.push(header);

        // Prominent location display
        column = column.push(
            widget::container(text(&self.config.location_name).size(18))
                .align_x(cosmic::iced::alignment::Horizontal::Center)
                .width(cosmic::iced::Length::Fill),
        );

        column = column.push(widget::divider::horizontal::default());

        // Show error message if there is one
        if let Some(ref error) = self.error_message {
            column = column.push(
                widget::container(
                    widget::column()
                        .spacing(10)
                        .push(widget::icon::from_name("dialog-error-symbolic").size(48))
                        .push(text(crate::fl!("failed-to-load")).size(18))
                        .push(text(error).size(14))
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
                    widget::column()
                        .spacing(10)
                        .align_x(cosmic::iced::alignment::Horizontal::Center)
                        .push(widget::icon::from_name("content-loading-symbolic").size(48))
                        .push(text(crate::fl!("loading")).size(18)),
                )
                .align_x(cosmic::iced::alignment::Horizontal::Center)
                .width(cosmic::iced::Length::Fill),
            );
        } else if self.showing_pollutants {
            // Pollutants sub-view replaces normal popup content
            column = column.push(self.render_pollutants_view());
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

    /// Application messages are handled here. The application state can be modified based on
    /// what message was received. Tasks may be returned for asynchronous execution on a
    /// background thread managed by the application's executor.
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
                let temp_unit = self.config.temperature_unit.api_param().to_string();
                let wind_unit = self
                    .config
                    .measurement_system
                    .wind_speed_api_param()
                    .to_string();
                let alerts_enabled = self.config.alerts_enabled;

                // Fetch weather and air quality in parallel
                let weather_task = Task::perform(
                    async move {
                        fetch_weather(lat, lon, &temp_unit, &wind_unit)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    |result| Action::App(Message::WeatherUpdated(result)),
                );

                let air_quality_task = Task::perform(
                    async move { fetch_air_quality(lat, lon).await.map_err(|e| e.to_string()) },
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

                return Task::batch([weather_task, air_quality_task, alerts_task]);
            }
            Message::WeatherUpdated(result) => {
                self.is_loading = false;

                match result {
                    Ok(data) => {
                        self.current_weathercode = data.current.weathercode;
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
                        self.current_weathercode = 0;
                        self.error_message = Some(e);
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
                Ok((lat, lon, location_name, country)) => {
                    self.config.latitude = lat;
                    self.config.longitude = lon;
                    self.config.location_name = location_name;

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
            Message::OpenKofi => {
                if let Err(e) = open::that("https://ko-fi.com/vintagetechie") {
                    tracing::error!("Failed to open Ko-fi URL: {}", e);
                }
            }
        }
        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced_runtime::Appearance> {
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

        if let Err(e) = Notification::new()
            .summary(&alert.event)
            .body(&alert.headline)
            .icon("weather-severe-alert")
            .urgency(urgency)
            .show()
        {
            tracing::warn!("Failed to send alert notification: {}", e);
        }
    }

    /// Creates a stat cell with label and bold value stacked vertically.
    fn stat_cell(label: String, value: String) -> Element<'static, Message> {
        let bold_font = cosmic::iced::Font {
            weight: cosmic::iced::font::Weight::Bold,
            ..Default::default()
        };
        widget::column()
            .spacing(2)
            .push(text(label).size(12))
            .push(text(value).size(14).font(bold_font))
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
        widget::row()
            .push(Self::stat_cell(left_label, left_value))
            .push(Self::stat_cell(right_label, right_value))
            .into()
    }

    /// Renders the Current weather tab content.
    fn render_current_tab(&self, weather: &WeatherData) -> Element<'_, Message> {
        let mut col = widget::column().spacing(16).padding([0, 8, 0, 20]);

        // Temperature and condition grouped together
        col = col.push(
            widget::column()
                .spacing(4)
                .push(
                    text(
                        self.config
                            .temperature_unit
                            .format(weather.current.temperature),
                    )
                    .size(36),
                )
                .push(text(weathercode_to_description(weather.current.weathercode)).size(14)),
        );

        col = col.push(widget::divider::horizontal::default());

        // Stats table
        let wind_unit = self.config.measurement_system.wind_speed_unit();
        let wind_dir = wind_direction_to_compass(weather.current.wind_direction);
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
            format!("{:.0} hPa", weather.current.pressure),
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

            let aqi_description = aqi_to_description(aq.aqi, aq.standard);
            let aqi_row = widget::button::custom(
                widget::row()
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(
                        widget::column()
                            .push(text(format!("{} {}", aq.aqi, aqi_description)).size(20))
                            .push(text(crate::fl!("air-quality-index")).size(12)),
                    )
                    .push(widget::horizontal_space())
                    .push(widget::icon::from_name("go-next-symbolic").size(16)),
            )
            .class(cosmic::theme::Button::Text)
            .on_press(Message::ShowPollutants);

            col = col.push(aqi_row);
        } else {
            col = col.push(widget::divider::horizontal::default());
            col = col.push(text(crate::fl!("air-quality-unavailable")).size(14));
        }

        col.into()
    }

    /// Renders the pollutants sub-view with Back button and pollutant list.
    fn render_pollutants_view(&self) -> Element<'_, Message> {
        let mut col = widget::column().spacing(8).padding([0, 8, 0, 20]);

        // Back button
        let back_btn = widget::button::custom(
            widget::row()
                .spacing(4)
                .align_y(cosmic::iced::Alignment::Center)
                .push(widget::icon::from_name("go-previous-symbolic").size(16))
                .push(text(crate::fl!("air-quality-back")).size(14)),
        )
        .class(cosmic::theme::Button::Link)
        .on_press(Message::HidePollutants);

        col = col.push(back_btn);
        col = col.push(widget::divider::horizontal::default());

        // Pollutant list
        if let Some(ref aq) = self.air_quality {
            col = col.push(Self::pollutant_row(
                crate::fl!("label-co"),
                format!("{:.1} ug/m3", aq.carbon_monoxide),
            ));
            col = col.push(Self::pollutant_row(
                crate::fl!("label-no2"),
                format!("{:.1} ug/m3", aq.nitrogen_dioxide),
            ));
            col = col.push(Self::pollutant_row(
                crate::fl!("label-ozone"),
                format!("{:.1} ug/m3", aq.ozone),
            ));
            col = col.push(Self::pollutant_row(
                crate::fl!("label-pm10"),
                format!("{:.1} ug/m3", aq.pm10),
            ));
            col = col.push(Self::pollutant_row(
                crate::fl!("label-pm25"),
                format!("{:.1} ug/m3", aq.pm2_5),
            ));
        }

        col.into()
    }

    /// Creates a row for a pollutant with label on left and value on right.
    fn pollutant_row(label: String, value: String) -> Element<'static, Message> {
        widget::row()
            .push(text(label).size(14))
            .push(widget::horizontal_space())
            .push(text(value).size(14))
            .into()
    }

    /// Renders the Alerts tab content.
    fn render_alerts_tab(&self) -> Element<'_, Message> {
        let mut col = widget::column().spacing(8).padding([0, 8, 0, 20]);

        if !self.config.alerts_enabled {
            col = col.push(
                widget::container(
                    widget::column()
                        .spacing(10)
                        .align_x(cosmic::iced::alignment::Horizontal::Center)
                        .push(text(crate::fl!("alerts-disabled")).size(14))
                        .push(text(crate::fl!("alerts-enable-hint")).size(12)),
                )
                .align_x(cosmic::iced::alignment::Horizontal::Center)
                .width(cosmic::iced::Length::Fill),
            );
        } else if self.alerts.is_empty() {
            col = col.push(
                widget::container(
                    widget::column()
                        .spacing(10)
                        .align_x(cosmic::iced::alignment::Horizontal::Center)
                        .push(
                            widget::icon::from_name("weather-clear-symbolic")
                                .size(48)
                                .symbolic(true),
                        )
                        .push(text(crate::fl!("no-active-alerts")).size(16))
                        .push(text(crate::fl!("area-clear")).size(12)),
                )
                .align_x(cosmic::iced::alignment::Horizontal::Center)
                .width(cosmic::iced::Length::Fill),
            );
        } else {
            for alert in &self.alerts {
                let severity_icon = match alert.severity {
                    AlertSeverity::Extreme => "dialog-error-symbolic",
                    AlertSeverity::Severe => "dialog-warning-symbolic",
                    AlertSeverity::Moderate => "dialog-information-symbolic",
                    _ => "weather-severe-alert-symbolic",
                };

                col = col.push(
                    widget::container(
                        widget::column()
                            .spacing(4)
                            .push(
                                widget::row()
                                    .spacing(8)
                                    .push(
                                        widget::icon::from_name(severity_icon)
                                            .size(20)
                                            .symbolic(true),
                                    )
                                    .push(text(&alert.event).size(14)),
                            )
                            .push(text(&alert.headline).size(12))
                            .push_maybe(if alert.description.is_empty() {
                                None
                            } else {
                                Some(
                                    widget::container(
                                        widget::scrollable(text(&alert.description).size(11))
                                            .height(cosmic::iced::Length::Fixed(100.0)),
                                    )
                                    .padding([4, 0, 4, 0]),
                                )
                            })
                            .push({
                                let time_fmt = if self.military_time {
                                    "%b %d %H:%M"
                                } else {
                                    "%b %d %I:%M %p"
                                };
                                let expires_time = alert.expires.format(time_fmt).to_string();
                                text(crate::fl!("expires", time = expires_time.as_str())).size(10)
                            }),
                    )
                    .padding(8)
                    .width(cosmic::iced::Length::Fill),
                );
                col = col.push(widget::divider::horizontal::default());
            }
        }

        col.into()
    }

    /// Renders the Hourly forecast tab content.
    fn render_hourly_tab(&self, weather: &WeatherData) -> Element<'_, Message> {
        let mut col = widget::column().spacing(8);
        let hours_per_row = 4;

        for chunk in weather.hourly.chunks(hours_per_row) {
            let mut row = widget::row().spacing(8);

            for hour in chunk {
                let cell = widget::column()
                    .spacing(4)
                    .align_x(cosmic::iced::alignment::Horizontal::Center)
                    .push(text(format_hour(&hour.time, self.military_time)).size(12))
                    .push(
                        widget::icon::from_name(weathercode_to_icon_name(hour.weathercode, false))
                            .size(20)
                            .symbolic(true),
                    )
                    .push(text(self.config.temperature_unit.format(hour.temperature)).size(14))
                    .push(text(format!("{}%", hour.precipitation_probability)).size(11));

                row = row.push(
                    widget::container(cell)
                        .width(cosmic::iced::Length::FillPortion(1))
                        .align_x(cosmic::iced::alignment::Horizontal::Center),
                );
            }

            // Pad incomplete rows
            for _ in chunk.len()..hours_per_row {
                row = row.push(
                    widget::container(widget::Space::new(0, 0))
                        .width(cosmic::iced::Length::FillPortion(1)),
                );
            }

            col = col.push(row);
        }

        col.into()
    }

    /// Renders the 7-day Forecast tab content.
    fn render_forecast_tab(&self, weather: &WeatherData) -> Element<'_, Message> {
        let mut col = widget::column()
            .spacing(8)
            .padding([0, 8, 0, 20])
            .width(cosmic::iced::Length::Fill);

        // Table header
        col = col.push(
            widget::row()
                .spacing(24)
                .push(
                    widget::container(text(crate::fl!("forecast-day")).size(12))
                        .width(cosmic::iced::Length::FillPortion(2)),
                )
                .push(widget::Space::new(20, 0))
                .push(
                    widget::container(text(crate::fl!("forecast-high")).size(12))
                        .width(cosmic::iced::Length::FillPortion(1)),
                )
                .push(
                    widget::container(text(crate::fl!("forecast-low")).size(12))
                        .width(cosmic::iced::Length::FillPortion(1)),
                )
                .push(
                    widget::container(text(crate::fl!("forecast-conditions")).size(12))
                        .width(cosmic::iced::Length::FillPortion(3)),
                ),
        );
        col = col.push(widget::divider::horizontal::default());

        // Data rows
        for day in &weather.forecast {
            col = col.push(
                widget::row()
                    .spacing(24)
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(
                        widget::container(text(format_date(&day.date)).size(14))
                            .width(cosmic::iced::Length::FillPortion(2)),
                    )
                    .push(
                        widget::icon::from_name(weathercode_to_icon_name(day.weathercode, false))
                            .size(20)
                            .symbolic(true),
                    )
                    .push(
                        widget::container(
                            text(self.config.temperature_unit.format(day.temp_max)).size(14),
                        )
                        .width(cosmic::iced::Length::FillPortion(1)),
                    )
                    .push(
                        widget::container(
                            text(self.config.temperature_unit.format(day.temp_min)).size(14),
                        )
                        .width(cosmic::iced::Length::FillPortion(1)),
                    )
                    .push(
                        widget::container(
                            text(weathercode_to_description(day.weathercode)).size(14),
                        )
                        .width(cosmic::iced::Length::FillPortion(3)),
                    ),
            );
        }

        col.into()
    }

    /// Creates a styled section header for the settings tab.
    fn section_header(label: String) -> Element<'static, Message> {
        text(label)
            .size(12)
            .class(cosmic::theme::Text::Accent)
            .into()
    }

    /// Renders the Settings tab content.
    fn render_settings_tab(&self) -> Element<'_, Message> {
        let mut col = widget::column().spacing(12).padding([0, 8, 0, 20]);

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
                widget::row()
                    .spacing(8)
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(
                        widget::column()
                            .push(text(&self.config.location_name).size(14))
                            .push(
                                text(crate::fl!("detected-via-ip"))
                                    .size(11)
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
                widget::row()
                    .spacing(8)
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

            // Search results
            for (idx, result) in self.search_results.iter().enumerate() {
                col = col.push(
                    widget::button::text(&result.display_name)
                        .on_press(Message::SelectLocation(idx))
                        .padding(8)
                        .width(cosmic::iced::Length::Fill),
                );
            }

            // Show current location with "Manually selected" subtitle
            col = col.push(
                widget::column()
                    .push(text(&self.config.location_name).size(14))
                    .push(
                        text(crate::fl!("manually-selected"))
                            .size(11)
                            .class(cosmic::theme::Text::Accent),
                    ),
            );
        }

        // UNITS section
        col = col.push(widget::divider::horizontal::default());
        col = col.push(Self::section_header(crate::fl!("section-units")));

        col = col.push(
            widget::row()
                .align_y(cosmic::iced::Alignment::Center)
                .push(text(crate::fl!("settings-temperature")).width(cosmic::iced::Length::Fill))
                .push(
                    widget::segmented_control::horizontal(&self.temperature_model)
                        .on_activate(Message::TemperatureUnitActivated),
                ),
        );

        col = col.push(
            widget::row()
                .align_y(cosmic::iced::Alignment::Center)
                .push(text(crate::fl!("settings-measurement")).width(cosmic::iced::Length::Fill))
                .push(
                    widget::segmented_control::horizontal(&self.measurement_model)
                        .on_activate(Message::MeasurementActivated),
                ),
        );

        col = col.push(settings::item(
            crate::fl!("settings-auto-units"),
            widget::row()
                .spacing(8)
                .align_y(cosmic::iced::Alignment::Center)
                .push(text(crate::fl!("settings-auto-units-hint")).size(11))
                .push(
                    widget::toggler(self.config.auto_units).on_toggle(|_| Message::ToggleAutoUnits),
                ),
        ));

        // UPDATES section
        col = col.push(widget::divider::horizontal::default());
        col = col.push(Self::section_header(crate::fl!("section-updates")));

        col = col.push(settings::item(
            crate::fl!("settings-refresh-interval"),
            widget::row()
                .spacing(8)
                .align_y(cosmic::iced::Alignment::Center)
                .push(text(crate::fl!("settings-min")).size(13))
                .push(
                    widget::text_input("15", &self.refresh_input)
                        .on_input(Message::UpdateRefreshInterval)
                        .width(cosmic::iced::Length::Fixed(60.0)),
                ),
        ));

        col = col.push(settings::item(
            crate::fl!("settings-weather-alerts"),
            widget::toggler(self.config.alerts_enabled).on_toggle(|_| Message::ToggleAlertsEnabled),
        ));

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
            widget::row()
                .align_y(cosmic::iced::Alignment::Center)
                .push(
                    text(format!("{} {}", crate::fl!("settings-version"), VERSION))
                        .size(13)
                        .class(cosmic::theme::Text::Accent),
                )
                .push(widget::horizontal_space())
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
            .min_width(440.0)
            .max_width(440.0)
            .min_height(180.0)
            .max_height(self.popup_max_height)
    }

    /// Sets temperature and measurement units based on country if auto_units is enabled.
    fn apply_units_for_country(&mut self, country: &str) {
        if self.config.auto_units {
            if uses_imperial_units(country) {
                self.config.temperature_unit = TemperatureUnit::Fahrenheit;
                self.config.measurement_system = MeasurementSystem::Imperial;
            } else {
                self.config.temperature_unit = TemperatureUnit::Celsius;
                self.config.measurement_system = MeasurementSystem::Metric;
            }
            // Sync the segmented control models with the new values
            self.temperature_model = build_temperature_model(self.config.temperature_unit);
            self.measurement_model = build_measurement_model(self.config.measurement_system);
        }
    }
}
