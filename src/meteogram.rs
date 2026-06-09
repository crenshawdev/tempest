// SPDX-License-Identifier: GPL-3.0-only

//! YR.no-style dual-panel meteogram, drawn on the codebase's first `iced` canvas.
//!
//! [`Meteogram`] implements [`canvas::Program`] and draws a fixed 260px-tall, 24-hour
//! weather chart from borrowed [`HourlyForecast`]/[`DailyForecast`] state only — no I/O,
//! consistent with the MVU `view()` contract. The top panel carries the temperature line
//! (with a gridlined, labeled axis) and auto-scaled precipitation bars; the bottom panel
//! carries the sustained- and gust-wind lines. Weather symbols, time labels, a now-marker
//! and per-hour night shading overlay both panels.
//!
//! All chrome colors resolve from the `Theme` draw parameter (D-10); only the four series
//! colors are fixed (D-09 — see the palette block below for the documented exception).

use chrono::{NaiveDate, NaiveDateTime};
use cosmic::iced::{Color, Point, Rectangle};
use cosmic::widget::canvas::{self, Geometry};

use crate::weather::{DailyForecast, HourlyForecast};

// ── Plot geometry (px) ────────────────────────────────────────────────────────
//
// These are canvas draw-math constants, NOT COSMIC `spacing()` widget tokens
// (UI-SPEC "Spacing Scale" exception): a single drawn surface lays itself out in
// raw pixels. All are multiples of 4 for crisp rendering. The vertical bands sum
// with the margins to the locked 260px canvas height:
//   MARGIN_TOP 24 + TOP_PANEL 120 + PANEL_GAP 8 + BOTTOM_PANEL 70
//   + AXIS_LABEL_GAP 16 + BOTTOM_MARGIN 22 = 260.

/// Temperature-axis label gutter (left of the plot rect).
const MARGIN_LEFT: f32 = 28.0;
/// Precip-peak label gutter (right of the plot rect), symmetric with the left.
const MARGIN_RIGHT: f32 = 28.0;
/// Weather-symbol row above the top panel (18px symbol + clearance).
const MARGIN_TOP: f32 = 24.0;
/// Vertical gap between the top (temp/precip) and bottom (wind) panels.
const PANEL_GAP: f32 = 8.0;
/// Time-axis label strip below the bottom panel.
const AXIS_LABEL_GAP: f32 = 16.0;

/// Top-panel plot height (temperature line + precipitation bars).
const TOP_PANEL: f32 = 120.0;
/// Bottom-panel plot height (wind sustained + gust lines).
const BOTTOM_PANEL: f32 = 70.0;

/// Number of hourly columns the chart plots.
const HOURS: usize = 24;
/// Cadence (in hours) for weather symbols and time-axis labels (every 3h → 8 marks).
const LABEL_STEP: usize = 3;

/// Width of a single precipitation bar (column width minus an inter-bar gap).
const BAR_WIDTH: f32 = 11.0;
/// Square side of an on-canvas weather symbol.
const SYMBOL_SIZE: f32 = 18.0;

// ── D-09 semantic series palette (FIXED, theme-independent) ─────────────────────
//
// DELIBERATE, DOCUMENTED EXCEPTION to CONVENTIONS.md:53 ("No custom styling;
// defers to system theme"). Chart series colors carry *meaning* (warm = heat,
// blue = water, a distinct hue = wind), so unlike all other applet colors they
// do NOT follow the theme accent — they are a fixed weather-semantic palette
// (D-09). Each series has a dark- and light-theme variant hand-tuned for
// contrast on both backgrounds. DO NOT "fix" these back to theme colors: that
// would erase the semantic encoding the chart depends on.

/// Temperature line — warm orange (dark theme variant).
const TEMP_DARK: Color = Color::from_rgb(
    0xFF as f32 / 255.0,
    0x8A as f32 / 255.0,
    0x4C as f32 / 255.0,
);
/// Temperature line — deeper orange (light theme variant, darkened for contrast).
const TEMP_LIGHT: Color = Color::from_rgb(
    0xE8 as f32 / 255.0,
    0x59 as f32 / 255.0,
    0x0C as f32 / 255.0,
);
/// Precipitation bars — sky blue (dark theme variant).
const PRECIP_DARK: Color = Color::from_rgb(
    0x4D as f32 / 255.0,
    0xA3 as f32 / 255.0,
    0xFF as f32 / 255.0,
);
/// Precipitation bars — deep blue (light theme variant).
const PRECIP_LIGHT: Color = Color::from_rgb(
    0x19 as f32 / 255.0,
    0x71 as f32 / 255.0,
    0xC2 as f32 / 255.0,
);
/// Wind sustained line — lavender/indigo (dark theme variant).
const WIND_DARK: Color = Color::from_rgb(
    0x9C as f32 / 255.0,
    0x8C as f32 / 255.0,
    0xFF as f32 / 255.0,
);
/// Wind sustained line — deep violet (light theme variant).
const WIND_LIGHT: Color = Color::from_rgb(
    0x67 as f32 / 255.0,
    0x41 as f32 / 255.0,
    0xD9 as f32 / 255.0,
);
/// Alpha applied to the wind hue for the gust line ("above sustained", D-03).
const GUST_ALPHA: f32 = 0.55;

/// The 24-hour meteogram canvas program.
///
/// Holds borrowed weather state only; all theme chrome is resolved from the
/// `Theme` draw parameter. The public field set is a cross-plan compile contract
/// — Plan 03's `view_window` constructs `Meteogram { hourly, daily, military_time }`
/// against exactly these names (`&Vec<T>` coerces to `&[T]`), so they must not
/// be renamed, reordered into owned `Vec`s, or folded into a `WeatherData` ref.
pub struct Meteogram<'a> {
    /// The 24 hourly entries (borrowed from `weather.hourly`).
    pub hourly: &'a [HourlyForecast],
    /// The daily slice, for per-hour sunrise/sunset (borrowed from `weather.forecast`).
    pub daily: &'a [DailyForecast],
    /// 12h/24h time-label formatting (mirrors the system preference).
    pub military_time: bool,
}

impl canvas::Program<crate::applet::Message, cosmic::Theme> for Meteogram<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &cosmic::Renderer,
        theme: &cosmic::Theme,
        bounds: Rectangle,
        _cursor: cosmic::iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let cosmic = theme.cosmic();
        let bg: Color = cosmic.background.base.into();

        let mut frame = canvas::Frame::new(renderer, bounds.size());

        // Background fill (D-10) — drawn whether or not there is data to plot.
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), bg);

        // Series drawing lands in Tasks 2/3.

        vec![frame.into_geometry()]
    }
}

/// Returns `true` if `hour_time` falls in night for the day that owns it.
///
/// Generalizes the single-instant `is_night_time` to an arbitrary hour: the 24h
/// window spans two calendar days, so each hour is classified against *its own*
/// day's sunrise/sunset (Pitfall 3). Returns `None` if anything fails to
/// parse/match — the caller then drops shading for that hour (D-07 graceful
/// degradation). No `unwrap`/`expect` touches the upstream timestamp strings.
fn hour_is_night(hour_time: &str, forecast: &[DailyForecast]) -> Option<bool> {
    let parse = |s: &str| {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M"))
            .ok()
    };
    let h = parse(hour_time)?;
    let h_date = h.date();
    let day = forecast
        .iter()
        .find(|d| NaiveDate::parse_from_str(&d.date, "%Y-%m-%d").ok() == Some(h_date))?;
    let sunrise = parse(&day.sunrise)?;
    let sunset = parse(&day.sunset)?;
    Some(h < sunrise || h > sunset)
}
