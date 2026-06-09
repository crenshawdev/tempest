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

// This module is standalone in Phase 2 Plan 02: it is fully drawable but not yet
// constructed anywhere. Plan 03's `view_window` builds `Meteogram { .. }` for the
// Graph tab, at which point `draw` and everything it reaches become live. Until
// then the whole call tree reads as dead code, so allow it module-wide rather than
// scatter per-item attributes that Plan 03 would immediately make redundant.
#![allow(dead_code)]

use chrono::{NaiveDate, NaiveDateTime};
use cosmic::iced::{alignment, Color, Pixels, Point, Rectangle, Size};
use cosmic::widget::canvas::{self, Geometry, Path, Stroke, Text};

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
const TEMP_DARK: Color = Color::from_rgb8(0xFF, 0x8A, 0x4C);
/// Temperature line — deeper orange (light theme variant, darkened for contrast).
const TEMP_LIGHT: Color = Color::from_rgb8(0xE8, 0x59, 0x0C);
/// Precipitation bars — sky blue (dark theme variant).
const PRECIP_DARK: Color = Color::from_rgb8(0x4D, 0xA3, 0xFF);
/// Precipitation bars — deep blue (light theme variant).
const PRECIP_LIGHT: Color = Color::from_rgb8(0x19, 0x71, 0xC2);
/// Wind sustained line — lavender/indigo (dark theme variant).
const WIND_DARK: Color = Color::from_rgb8(0x9C, 0x8C, 0xFF);
/// Wind sustained line — deep violet (light theme variant).
const WIND_LIGHT: Color = Color::from_rgb8(0x67, 0x41, 0xD9);
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

    // Hour indices and counts are ≤24, well within f32's exact-integer range, so the
    // precision-loss lint does not apply to these casts.
    #[allow(clippy::too_many_lines, clippy::cast_precision_loss)]
    fn draw(
        &self,
        _state: &Self::State,
        renderer: &cosmic::Renderer,
        theme: &cosmic::Theme,
        bounds: Rectangle,
        _cursor: cosmic::iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let cosmic = theme.cosmic();
        let is_dark = cosmic.is_dark;
        let bg: Color = cosmic.background.base.into();
        let on: Color = cosmic.background.on.into();
        // Theme-resolved chrome alphas (D-10 / UI-SPEC "Chart chrome palette").
        let night = with_alpha(on, 0.06);
        let gridline = with_alpha(on, 0.12);
        let label = with_alpha(on, 0.70);

        let mut frame = canvas::Frame::new(renderer, bounds.size());

        // 1. Background fill (D-10) — drawn whether or not there is data to plot.
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), bg);

        // Nothing to plot without hours — leave the blank themed surface.
        let n = self.hourly.len();
        if n == 0 {
            return vec![frame.into_geometry()];
        }

        // Horizontal plot geometry (read bounds, never hardcode 416 — anti-pattern A4).
        let plot_w = (bounds.width - MARGIN_LEFT - MARGIN_RIGHT).max(1.0);
        let col_w = plot_w / HOURS as f32;
        // Column center x for hour index `h`.
        let cx = |h: usize| MARGIN_LEFT + (h as f32 + 0.5) * col_w;

        // Vertical bands: symbol row, then the top (temp/precip) plot rect.
        let top_y0 = MARGIN_TOP;
        let top_y1 = MARGIN_TOP + TOP_PANEL;

        // 2. Night shading (D-07): full-height bands behind night columns. Drop a
        // band entirely when classification fails (None) — never guess.
        for (h, hour) in self.hourly.iter().enumerate() {
            if hour_is_night(&hour.time, self.daily) == Some(true) {
                let x = MARGIN_LEFT + h as f32 * col_w;
                frame.fill_rectangle(Point::new(x, 0.0), Size::new(col_w, bounds.height), night);
            }
        }

        // ── Temperature axis (left) — auto-scale to 24h min/max with ~10% headroom.
        let temps: Vec<f32> = self
            .hourly
            .iter()
            .map(|h| h.temperature)
            .filter(|t| t.is_finite())
            .collect();
        if let Some((t_min, t_max)) = min_max(&temps) {
            let pad = ((t_max - t_min) * 0.1).max(0.5);
            let lo = t_min - pad;
            let hi = t_max + pad;
            let span = (hi - lo).max(f32::EPSILON);
            // Map a temperature to a y within the top panel (higher temp → higher up).
            let temp_y = |t: f32| top_y1 - ((t - lo) / span) * TOP_PANEL;

            // 3. Temperature gridlines (D-08) at "nice" rounded values, theme on@12%.
            for value in nice_gridlines(lo, hi) {
                let y = temp_y(value);
                let line = Path::new(|b| {
                    b.move_to(Point::new(MARGIN_LEFT, y));
                    b.line_to(Point::new(bounds.width - MARGIN_RIGHT, y));
                });
                frame.stroke(
                    &line,
                    Stroke::default().with_width(1.0).with_color(gridline),
                );
                // Gridline value label in the left gutter (11px, on@70%).
                frame.fill_text(Text {
                    content: format!("{value:.0}"),
                    position: Point::new(MARGIN_LEFT - 4.0, y),
                    color: label,
                    size: Pixels(11.0),
                    align_x: alignment::Horizontal::Right.into(),
                    align_y: alignment::Vertical::Center,
                    ..Text::default()
                });
            }

            // 4. Temperature line (GRAPH-02): 2px polyline through column centers,
            // D-09 temp color chosen by theme brightness. Skip non-finite points.
            let temp_color = if is_dark { TEMP_DARK } else { TEMP_LIGHT };
            let line = Path::new(|b| {
                let mut started = false;
                for (h, hour) in self.hourly.iter().enumerate() {
                    if !hour.temperature.is_finite() {
                        continue;
                    }
                    let p = Point::new(cx(h), temp_y(hour.temperature));
                    if started {
                        b.line_to(p);
                    } else {
                        b.move_to(p);
                        started = true;
                    }
                }
            });
            frame.stroke(
                &line,
                Stroke::default().with_width(2.0).with_color(temp_color),
            );
        }

        // 5. Precipitation bars (GRAPH-03): auto-scale to the 24h max but never
        // below the floor, so a drizzle doesn't fill the panel (D-02). The locked
        // Meteogram contract carries no measurement-system field, so the floor uses
        // the metric 2mm value; weathervane already delivers `precipitation` in the
        // user's unit, so an imperial window simply scales against the (smaller)
        // numeric floor — the auto-scale still bounds the panel correctly.
        let precip_floor = 2.0_f32;
        let precip_max = self
            .hourly
            .iter()
            .map(|h| h.precipitation)
            .filter(|p| p.is_finite())
            .fold(0.0_f32, f32::max);
        let precip_scale = precip_max.max(precip_floor).max(f32::EPSILON);
        let precip_color = if is_dark { PRECIP_DARK } else { PRECIP_LIGHT };
        for (h, hour) in self.hourly.iter().enumerate() {
            let p = hour.precipitation;
            if !p.is_finite() || p <= 0.0 {
                continue;
            }
            let frac = (p / precip_scale).clamp(0.0, 1.0);
            let bar_h = frac * TOP_PANEL;
            if bar_h <= 0.0 {
                continue;
            }
            let x = cx(h) - BAR_WIDTH / 2.0;
            frame.fill_rectangle(
                Point::new(x, top_y1 - bar_h),
                Size::new(BAR_WIDTH, bar_h),
                precip_color,
            );
        }

        // 6. Precip peak label — the actual window max (in the user's unit, which is
        // what weathervane already returns), drawn top-right. No unit suffix: the
        // locked Meteogram contract has no measurement-system field to resolve mm vs
        // in, and the Hourly tab already carries the unit string for the same data.
        if precip_max > 0.0 {
            frame.fill_text(Text {
                content: format!("{precip_max:.1}"),
                position: Point::new(bounds.width - MARGIN_RIGHT + 4.0, top_y0),
                color: label,
                size: Pixels(11.0),
                align_x: alignment::Horizontal::Left.into(),
                align_y: alignment::Vertical::Top,
                ..Text::default()
            });
        }

        // Bottom wind panel, symbols, time axis and now-marker land in Task 3.

        vec![frame.into_geometry()]
    }
}

/// Returns `color` with its alpha replaced by `a` (theme chrome dimming).
fn with_alpha(color: Color, a: f32) -> Color {
    Color { a, ..color }
}

/// Finite min/max of a slice, or `None` if it is empty.
fn min_max(values: &[f32]) -> Option<(f32, f32)> {
    let mut iter = values.iter().copied();
    let first = iter.next()?;
    Some(iter.fold((first, first), |(lo, hi), v| (lo.min(v), hi.max(v))))
}

/// 3–4 "nice" rounded gridline values spanning `[lo, hi]` for the temperature axis.
///
/// Picks a round step (1/2/5 × 10ⁿ) targeting ~3 intervals, then walks the rounded
/// multiples that fall inside the range. Guards against a zero/degenerate span so an
/// all-equal temperature series still yields a single labeled line rather than a
/// divide-by-zero or an unbounded loop.
fn nice_gridlines(lo: f32, hi: f32) -> Vec<f32> {
    let span = hi - lo;
    if !span.is_finite() || span <= f32::EPSILON {
        return vec![lo];
    }
    let raw = span / 3.0;
    let mag = 10.0_f32.powf(raw.log10().floor());
    let norm = raw / mag;
    let step = if norm < 1.5 {
        1.0
    } else if norm < 3.0 {
        2.0
    } else if norm < 7.0 {
        5.0
    } else {
        10.0
    } * mag;
    let start = (lo / step).ceil() * step;
    let mut out = Vec::new();
    let mut v = start;
    // At most ~8 lines; the bound also protects against any FP edge case.
    while v <= hi + f32::EPSILON && out.len() < 8 {
        out.push(v);
        v += step;
    }
    if out.is_empty() {
        out.push(lo);
    }
    out
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
