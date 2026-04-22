// SPDX-License-Identifier: GPL-3.0-only

//! Thin i18n adapters over tempest-core.
//!
//! The core library returns typed enums. This module matches on them
//! to produce localized strings via the applet's Fluent translations.

use weathervane::{AqiCategory, EuAqiCategory, UsAqiCategory, WeatherCondition};

// Re-export everything the rest of the applet needs from tempest-core.
pub use weathervane::{
    detect_location, detect_region, fetch_air_quality, fetch_alerts, fetch_weather, format_hour,
    format_time, is_night_time, search_city, uses_imperial_units, AirQualityData, Alert,
    AlertSeverity, AqiStandard, DetectedLocation, LocationResult, Region, WeatherData,
};

/// Localized description for a weather condition.
pub fn condition_to_description(condition: WeatherCondition) -> String {
    match condition {
        WeatherCondition::ClearSky => crate::fl!("weather-clear-sky"),
        WeatherCondition::MainlyClear => crate::fl!("weather-mainly-clear"),
        WeatherCondition::PartlyCloudy => crate::fl!("weather-partly-cloudy"),
        WeatherCondition::Overcast => crate::fl!("weather-overcast"),
        WeatherCondition::Foggy => crate::fl!("weather-foggy"),
        WeatherCondition::Drizzle => crate::fl!("weather-drizzle"),
        WeatherCondition::FreezingDrizzle => crate::fl!("weather-freezing-drizzle"),
        WeatherCondition::Rain => crate::fl!("weather-rain"),
        WeatherCondition::FreezingRain => crate::fl!("weather-freezing-rain"),
        WeatherCondition::Snow => crate::fl!("weather-snow"),
        WeatherCondition::SnowGrains => crate::fl!("weather-snow-grains"),
        WeatherCondition::RainShowers => crate::fl!("weather-rain-showers"),
        WeatherCondition::SnowShowers => crate::fl!("weather-snow-showers"),
        WeatherCondition::Thunderstorm => crate::fl!("weather-thunderstorm"),
        WeatherCondition::ThunderstormHail => crate::fl!("weather-thunderstorm-hail"),
        WeatherCondition::Unknown => crate::fl!("weather-unknown"),
    }
}

/// Localized AQI description based on category.
pub fn aqi_to_description(category: &AqiCategory) -> String {
    match category {
        AqiCategory::Us(cat) => match cat {
            UsAqiCategory::Good => crate::fl!("aqi-us-good"),
            UsAqiCategory::Moderate => crate::fl!("aqi-us-moderate"),
            UsAqiCategory::UnhealthySensitive => crate::fl!("aqi-us-unhealthy-sensitive"),
            UsAqiCategory::Unhealthy => crate::fl!("aqi-us-unhealthy"),
            UsAqiCategory::VeryUnhealthy => crate::fl!("aqi-us-very-unhealthy"),
            UsAqiCategory::Hazardous => crate::fl!("aqi-us-hazardous"),
        },
        AqiCategory::Eu(cat) => match cat {
            EuAqiCategory::Good => crate::fl!("aqi-eu-good"),
            EuAqiCategory::Fair => crate::fl!("aqi-eu-fair"),
            EuAqiCategory::Moderate => crate::fl!("aqi-eu-moderate"),
            EuAqiCategory::Poor => crate::fl!("aqi-eu-poor"),
            EuAqiCategory::VeryPoor => crate::fl!("aqi-eu-very-poor"),
            EuAqiCategory::ExtremelyPoor => crate::fl!("aqi-eu-extremely-poor"),
        },
    }
}

/// Formats an ISO date to a localized readable string (e.g. "Tue Nov 25").
pub fn format_date(date_str: &str) -> String {
    if let Some(parsed) = weathervane::ParsedDate::from_iso(date_str) {
        let day_name = match parsed.weekday {
            chrono::Weekday::Mon => crate::fl!("day-mon"),
            chrono::Weekday::Tue => crate::fl!("day-tue"),
            chrono::Weekday::Wed => crate::fl!("day-wed"),
            chrono::Weekday::Thu => crate::fl!("day-thu"),
            chrono::Weekday::Fri => crate::fl!("day-fri"),
            chrono::Weekday::Sat => crate::fl!("day-sat"),
            chrono::Weekday::Sun => crate::fl!("day-sun"),
        };
        let month_name = match parsed.month {
            1 => crate::fl!("month-jan"),
            2 => crate::fl!("month-feb"),
            3 => crate::fl!("month-mar"),
            4 => crate::fl!("month-apr"),
            5 => crate::fl!("month-may"),
            6 => crate::fl!("month-jun"),
            7 => crate::fl!("month-jul"),
            8 => crate::fl!("month-aug"),
            9 => crate::fl!("month-sep"),
            10 => crate::fl!("month-oct"),
            11 => crate::fl!("month-nov"),
            12 => crate::fl!("month-dec"),
            _ => unreachable!(),
        };
        format!("{day_name} {month_name} {:02}", parsed.day)
    } else {
        date_str.to_string()
    }
}
