// SPDX-License-Identifier: GPL-3.0-only

//! Thin i18n adapters over weathervane.
//!
//! The core library returns typed enums. This module matches on them
//! to produce localized strings via the applet's Fluent translations.

use weathervane::{AqiCategory, EuAqiCategory, UsAqiCategory, WeatherCondition};

/// Pollen severity bucket, aligned to the European Aeroallergen Network (EAN) scale
/// that CAMS data is calibrated against.
///
/// `OffSeason` is added on top of the four EAN tiers to preserve weathervane's
/// `PollenData` semantics: a reading of `0.0` means the species is not actively
/// producing pollen at this location right now (off-season, or not regionally
/// present), which is distinct from "Low" on the EAN scale. Collapsing 0.0 to
/// Low would surface every species year-round and bury the signal.
///
/// Declaration order doubles as severity ordering — the derived `Ord` lets the
/// UI pick the highest-severity active species with `iter().max()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PollenLevel {
    OffSeason,
    Low,
    Moderate,
    High,
    VeryHigh,
}

/// Pollen species the API reports. Mirrors the field names on `weathervane::PollenData`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollenSpecies {
    Alder,
    Birch,
    Grass,
    Mugwort,
    Olive,
    Ragweed,
}

/// Categorizes a grains/m³ reading for a given species using EAN thresholds.
///
/// Threshold families: trees (alder, birch), grasses and weeds (grass, mugwort,
/// ragweed), and olive on its own scale because olives shed at volumes between
/// the other two. 0.0 always collapses to `OffSeason` regardless of species.
#[must_use]
pub fn categorize_pollen(species: PollenSpecies, grains: f32) -> PollenLevel {
    if grains <= 0.0 {
        return PollenLevel::OffSeason;
    }
    match species {
        PollenSpecies::Alder | PollenSpecies::Birch => match grains {
            g if g <= 10.0 => PollenLevel::Low,
            g if g <= 100.0 => PollenLevel::Moderate,
            g if g <= 1000.0 => PollenLevel::High,
            _ => PollenLevel::VeryHigh,
        },
        PollenSpecies::Grass | PollenSpecies::Mugwort | PollenSpecies::Ragweed => match grains {
            g if g <= 5.0 => PollenLevel::Low,
            g if g <= 20.0 => PollenLevel::Moderate,
            g if g <= 50.0 => PollenLevel::High,
            _ => PollenLevel::VeryHigh,
        },
        PollenSpecies::Olive => match grains {
            g if g <= 10.0 => PollenLevel::Low,
            g if g <= 50.0 => PollenLevel::Moderate,
            g if g <= 200.0 => PollenLevel::High,
            _ => PollenLevel::VeryHigh,
        },
    }
}

/// Single source of the per-species pollen reading list.
///
/// Returns each of the six API-reported species paired with its raw grains/m³
/// value from `data`, in natural species order (Alder, Birch, Grass, Mugwort,
/// Olive, Ragweed). This is a PLAIN data table — it performs no categorization
/// or filtering; callers apply `categorize_pollen` and any `OffSeason` filtering
/// themselves. It is consumed by both the panel summary (which filters to the
/// active species) and the pollen sub-view (which formats every reading).
#[must_use]
pub fn species_readings(data: &PollenData) -> [(PollenSpecies, f32); 6] {
    [
        (PollenSpecies::Alder, data.alder),
        (PollenSpecies::Birch, data.birch),
        (PollenSpecies::Grass, data.grass),
        (PollenSpecies::Mugwort, data.mugwort),
        (PollenSpecies::Olive, data.olive),
        (PollenSpecies::Ragweed, data.ragweed),
    ]
}

// Re-export everything the rest of the applet needs from tempest-core.
pub use weathervane::{
    detect_location, detect_region, fetch_air_quality, fetch_alerts, fetch_pollen, fetch_weather,
    format_hour, format_time, is_night_time, search_city, uses_imperial_units, AirQualityData,
    Alert, AlertSeverity, AqiStandard, DailyForecast, DetectedLocation, HourlyForecast,
    LocationResult, PollenData, Region, WeatherData,
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

/// Localized description for a pollen severity level.
pub fn pollen_level_to_description(level: PollenLevel) -> String {
    match level {
        PollenLevel::OffSeason => crate::fl!("pollen-level-off-season"),
        PollenLevel::Low => crate::fl!("pollen-level-low"),
        PollenLevel::Moderate => crate::fl!("pollen-level-moderate"),
        PollenLevel::High => crate::fl!("pollen-level-high"),
        PollenLevel::VeryHigh => crate::fl!("pollen-level-very-high"),
    }
}

/// Localized common name for a pollen species.
pub fn pollen_species_to_description(species: PollenSpecies) -> String {
    match species {
        PollenSpecies::Alder => crate::fl!("pollen-species-alder"),
        PollenSpecies::Birch => crate::fl!("pollen-species-birch"),
        PollenSpecies::Grass => crate::fl!("pollen-species-grass"),
        PollenSpecies::Mugwort => crate::fl!("pollen-species-mugwort"),
        PollenSpecies::Olive => crate::fl!("pollen-species-olive"),
        PollenSpecies::Ragweed => crate::fl!("pollen-species-ragweed"),
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

#[cfg(test)]
mod tests {
    use super::*;

    // Fixtures from the live pollen probe in spike 001 (Berlin, Paris, Rome on
    // 2026-05-13). The EAN scale partitions these the way a Roman allergy
    // sufferer would expect: 19.1 grass is moderate, not "very high."
    #[test]
    fn categorizes_live_fixtures_against_ean_scale() {
        assert_eq!(
            categorize_pollen(PollenSpecies::Grass, 19.1),
            PollenLevel::Moderate
        );
        assert_eq!(
            categorize_pollen(PollenSpecies::Grass, 0.7),
            PollenLevel::Low
        );
        assert_eq!(
            categorize_pollen(PollenSpecies::Grass, 0.2),
            PollenLevel::Low
        );
        assert_eq!(
            categorize_pollen(PollenSpecies::Olive, 0.6),
            PollenLevel::Low
        );
    }

    #[test]
    fn zero_collapses_to_off_season_for_every_species() {
        for species in [
            PollenSpecies::Alder,
            PollenSpecies::Birch,
            PollenSpecies::Grass,
            PollenSpecies::Mugwort,
            PollenSpecies::Olive,
            PollenSpecies::Ragweed,
        ] {
            assert_eq!(categorize_pollen(species, 0.0), PollenLevel::OffSeason);
        }
    }

    #[test]
    fn tree_scale_boundaries() {
        // EAN tree thresholds: <=10 Low, <=100 Moderate, <=1000 High, else Very High.
        assert_eq!(
            categorize_pollen(PollenSpecies::Birch, 10.0),
            PollenLevel::Low
        );
        assert_eq!(
            categorize_pollen(PollenSpecies::Birch, 10.01),
            PollenLevel::Moderate
        );
        assert_eq!(
            categorize_pollen(PollenSpecies::Birch, 100.0),
            PollenLevel::Moderate
        );
        assert_eq!(
            categorize_pollen(PollenSpecies::Birch, 100.01),
            PollenLevel::High
        );
        assert_eq!(
            categorize_pollen(PollenSpecies::Birch, 1000.0),
            PollenLevel::High
        );
        assert_eq!(
            categorize_pollen(PollenSpecies::Birch, 1000.1),
            PollenLevel::VeryHigh
        );
    }

    #[test]
    fn grass_weed_scale_boundaries() {
        // EAN grass/weed thresholds: <=5 Low, <=20 Moderate, <=50 High, else Very High.
        assert_eq!(
            categorize_pollen(PollenSpecies::Grass, 5.0),
            PollenLevel::Low
        );
        assert_eq!(
            categorize_pollen(PollenSpecies::Grass, 20.0),
            PollenLevel::Moderate
        );
        assert_eq!(
            categorize_pollen(PollenSpecies::Grass, 50.0),
            PollenLevel::High
        );
        assert_eq!(
            categorize_pollen(PollenSpecies::Grass, 50.1),
            PollenLevel::VeryHigh
        );
        assert_eq!(
            categorize_pollen(PollenSpecies::Ragweed, 50.1),
            PollenLevel::VeryHigh
        );
    }

    #[test]
    fn olive_scale_boundaries() {
        // EAN olive thresholds: <=10 Low, <=50 Moderate, <=200 High, else Very High.
        assert_eq!(
            categorize_pollen(PollenSpecies::Olive, 10.0),
            PollenLevel::Low
        );
        assert_eq!(
            categorize_pollen(PollenSpecies::Olive, 200.0),
            PollenLevel::High
        );
        assert_eq!(
            categorize_pollen(PollenSpecies::Olive, 200.1),
            PollenLevel::VeryHigh
        );
    }
}
