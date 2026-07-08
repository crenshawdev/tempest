// SPDX-License-Identifier: GPL-3.0-only

//! Tempest's relocated view layer: the panel-button view, the popup-window view,
//! and the per-tab / per-section render helpers.
//!
//! These methods hang off [`Tempest`] (a child module of `applet`, so its
//! `impl Tempest` block reads the parent struct's private fields) and draw from
//! borrowed state only — no I/O — consistent with the MVU `view()` contract.
//! This module is a pure relocation out of `applet.rs` with no behavior change:
//! the moved bodies are byte-identical to their originals, and the trait
//! `view()`/`view_window()` methods in `applet.rs` delegate here via one-line shims.

use cosmic::iced::window::Id;
use cosmic::widget::{self, settings};
use cosmic::Element;

use crate::config::PopupTab;
use crate::weather::{
    aqi_to_description, categorize_pollen, condition_to_description, format_date, format_hour,
    format_time, is_night_time, pollen_level_to_description, pollen_species_to_description,
    AlertSeverity, AqiSource, PollenData, PollenLevel, PollenSpecies, WeatherData,
};

use crate::applet::{Message, Tempest, VERSION};

const UG_PER_M3: &str = "µg/m³";

/// Returns the species in `data` with a non-zero, non-`OffSeason` reading,
/// paired with the raw grains/m³ value and the EAN severity bucket. Sorted
/// in the natural species order; callers pick the leader with
/// `iter().max_by_key(|(_, _, level)| *level)`.
fn active_pollen_species(data: &PollenData) -> Vec<(PollenSpecies, f32, PollenLevel)> {
    crate::weather::species_readings(data)
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

impl Tempest {
    /// Renders the panel button (relocated body of the trait `view()`).
    pub(crate) fn view_panel(&self) -> Element<'_, Message> {
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

        // Use the error icon only when there's truly no weather to show. A
        // transient refresh failure keeps the last good condition icon so the
        // panel doesn't flicker to an error glyph on every network blip.
        let icon_name = if self.error_message.is_some() && self.weather_data.is_none() {
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

    /// Renders the popup window (relocated body of the trait `view_window()`).
    pub(crate) fn view_popup(&self, _id: Id) -> Element<'_, Message> {
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

        // A failed refresh that still has cached weather annotates the timestamp
        // in place ("· couldn't refresh" + a small warning glyph) instead of a
        // separate banner, keeping the lowest-footprint staleness cue in the
        // fixed-width header.
        if self.refresh_failed {
            header = header
                .push(widget::text::caption(crate::fl!("couldnt-refresh")))
                .push(
                    widget::icon::from_name("dialog-warning-symbolic")
                        .size(12)
                        .symbolic(true),
                );
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

            // Attribution reflects the source weathervane actually used, read
            // straight off the data (`aqi_source`) instead of re-deriving the
            // token/region selection logic here.
            match aq.aqi_source {
                AqiSource::Aqicn => {
                    col = col.push(widget::text::caption(crate::fl!("aqicn-attribution")));
                }
                AqiSource::OpenMeteo => {
                    col = col.push(widget::text::caption(crate::fl!("openmeteo-attribution")));
                }
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
            let rows = crate::weather::species_readings(p);

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

        // Wind unit (km/h or mph) matches the Current tab derivation; precipitation
        // amount unit (mm or in) is derived from the measurement system the same way.
        // Both are loop-invariant, so they are computed once above the per-chunk loop.
        let wind_unit = self.config.measurement_system.wind_speed_unit();
        let precip_unit = self.config.measurement_system.precipitation_unit();

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
                    // Per-hour wind speed (already converted to the user's unit by weathervane)
                    .push(widget::text::caption(format!(
                        "{:.0} {wind_unit}",
                        hour.windspeed
                    )))
                    // Per-hour precipitation amount (in the user's unit)
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

    /// Renders the Graph tab: the YR.no-style meteogram canvas above an
    /// always-visible series legend.
    ///
    /// The canvas is constructed against `meteogram::Meteogram`'s field
    /// contract (`hourly` / `daily` / `military_time`; `&Vec<T>` coerces to the
    /// struct's `&[T]` fields). Height MUST be a `Fixed` value — `Shrink` collapses
    /// the canvas to zero inside the surrounding `scrollable`; 300px
    /// matches the meteogram's band-height constants (grown from 260px so the panels
    /// and time labels aren't cramped). Width fills the ~416px popup content area.
    ///
    /// The result is a `Column [canvas, legend_row]`; the `canvas::Cache` is
    /// untouched (the legend is iced widgets, not canvas geometry). The legend row
    /// is pushed only when there are hours to plot, mirroring the canvas `n == 0`
    /// early return in `meteogram.rs` — so legend and chart appear/vanish together.
    fn render_graph_tab<'a>(&'a self, weather: &'a WeatherData) -> Element<'a, Message> {
        let spacing = cosmic::theme::spacing();
        // Precip peak-label unit, derived the same way as the enriched Hourly cell.
        let precip_unit = self.config.measurement_system.precipitation_unit();
        let canvas = cosmic::widget::Canvas::new(crate::meteogram::Meteogram {
            cache: &self.meteogram_cache,
            hourly: &weather.hourly,
            daily: &weather.forecast,
            military_time: self.military_time,
            precip_unit,
        })
        .width(cosmic::iced::Length::Fill)
        .height(cosmic::iced::Length::Fixed(300.0));

        let mut col = widget::Column::new()
            .spacing(spacing.space_xxs)
            .push(canvas);
        // LEGEND-05 data-missing guard: no hours → blank canvas → no legend.
        if !weather.hourly.is_empty() {
            col = col.push(Self::render_legend_row());
        }
        col.into()
    }

    /// The always-visible four-entry meteogram legend as a single centered
    /// row — Temperature, Precipitation, Wind, Gust in fixed order — rendered
    /// directly on the popup `background.base` below the canvas (no card surface,
    /// so the 55%-alpha Gust mark composites to the same color as the chart).
    fn render_legend_row() -> Element<'static, Message> {
        let spacing = cosmic::theme::spacing();
        let row = widget::Row::new()
            .spacing(spacing.space_s)
            .align_y(cosmic::iced::Alignment::Center)
            .push(Self::legend_entry(0, crate::fl!("legend-temperature")))
            .push(Self::legend_entry(1, crate::fl!("legend-precipitation")))
            .push(Self::legend_entry(2, crate::fl!("legend-wind")))
            .push(Self::legend_entry(3, crate::fl!("legend-gust")));
        // Center under the full popup content width: the canvas plot area is inset
        // by the y-axis label gutter, so a left-packed row read as lopsided at 480px.
        widget::container(row)
            .width(cosmic::iced::Length::Fill)
            .align_x(cosmic::iced::alignment::Horizontal::Center)
            .into()
    }

    /// One legend entry: an 18×12 series *mark* bound to its label.
    ///
    /// `idx` indexes the fixed `[Temperature, Precipitation, Wind, Gust]` order.
    /// The mark is a [`crate::meteogram::LegendMark`] canvas that draws the same
    /// shape the chart draws for that series — a solid line (Temperature, Wind), a
    /// dashed line (Gust), or a filled bar (Precipitation) — resolving its color
    /// from `legend_colors` inside its own `draw`, so it re-reads the live theme on
    /// a light/dark switch and stays locked to the chart (no palette literals leak
    /// into the view). The mark shape is what disambiguates the near-identical
    /// solid-wind and dashed-gust hues.
    fn legend_entry(idx: usize, label: String) -> Element<'static, Message> {
        let spacing = cosmic::theme::spacing();
        let mark = cosmic::widget::Canvas::new(crate::meteogram::LegendMark { idx })
            .width(cosmic::iced::Length::Fixed(18.0))
            .height(cosmic::iced::Length::Fixed(12.0));
        widget::Row::new()
            .spacing(spacing.space_xxxs)
            .align_y(cosmic::iced::Alignment::Center)
            .push(mark)
            .push(widget::text::body(label))
            .into()
    }

    /// Renders the 7-day Forecast tab content.
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

            // Inline feedback when the 8-slot save cap is reached.
            if self.saved_locations_full {
                section = section.add(widget::text::caption(crate::fl!("saved-locations-full")));
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
            let is_active = location.matches_coords(self.config.latitude, self.config.longitude);

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
                            .on_input(Message::UpdateRefreshInterval)
                            .on_submit(|_| Message::CommitRefreshInterval),
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
                            .on_submit(|_| Message::CommitAqicnToken)
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

    /// SUPPORT section: version label, tip-jar, source, and issue-tracker buttons.
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
            .add(
                widget::Row::new()
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(
                        widget::button::link(crate::fl!("settings-source-code"))
                            .on_press(Message::OpenSourceCode),
                    )
                    .push(widget::space::horizontal())
                    .push(
                        widget::button::link(crate::fl!("settings-report-issue"))
                            .on_press(Message::OpenWorkItems),
                    ),
            )
            .into()
    }
}
