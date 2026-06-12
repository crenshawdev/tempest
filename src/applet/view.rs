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
use cosmic::widget::{self, canvas, settings};
use cosmic::Element;

use crate::config::PopupTab;
use crate::weather::{
    aqi_to_description, categorize_pollen, condition_to_description, format_date, format_hour,
    format_time, is_night_time, pollen_level_to_description, pollen_species_to_description,
    AlertSeverity, AqiSource, PollenData, PollenLevel, PollenSpecies, WeatherData,
};

use crate::applet::{Message, Tempest, VERSION};

const UG_PER_M3: &str = "µg/m³";

impl Tempest {}
