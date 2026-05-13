# Spike Manifest

## Idea

Integrate weathervane v0.4.0's new pollen feature into the cosmic-ext-applet-tempest popup. The library exposes `fetch_pollen(lat, lon) -> Result<Option<PollenData>>` returning six species (alder, birch, grass, mugwort, olive, ragweed) in grains/m³, sourced from the CAMS European Air Quality Forecast model via Open-Meteo. Coverage is EU-only. Non-EU coordinates get `Ok(None)` instead of an error, so pollen is a region-optional feature like alerts. Investigate API surface, decide a categorization scheme that turns raw grains/m³ into something a human can act on, validate the EU coverage boundary, and propose a UI placement that fits COSMIC's design vocabulary already in use.

Continues issue #126 (Mattermost feature request for pollen support).

## Requirements

Tracked as they emerge from user choices during spiking.

- Region-aware: do not show pollen UI for users outside CAMS coverage (graceful absence, not "unavailable" stub)
- **Do NOT gate the pollen fetch on `Region::Europe`.** CAMS coverage is wider than weathervane's MeteoAlarm/AQI bbox — Tel Aviv, Cairo, and east Turkey all return real data. Call `fetch_pollen` unconditionally and let `Ok(None)` be the gate. (See Spike 004.)
- Honor weathervane's `Ok(None)` contract — pollen is optional, not required
- Reuse the existing AQI drill-down pattern (summary in Current → chevron → sub-view) if UI placement spike confirms it as the best fit
- i18n: any new strings land in `i18n/en/tempest.ftl` and `i18n/en-US/tempest.ftl` only; Weblate handles the rest
- No new top-level tab in the segmented control (Current / Hourly / 7-Day stays)
- Severity wording must reference a public-health-grounded scale, not invented thresholds
- Use the **EAN scale** (European Aeroallergen Network, what CAMS data is calibrated to) — not AAAAI/NAB which would be wrong for EU data
- Surface a `PollenLevel` enum with five variants: `OffSeason / Low / Moderate / High / VeryHigh` — 0.0 readings always collapse to `OffSeason`
- Default to showing only **non-`OffSeason` species** in the popup (mid-May real data shows only 1-2 species active per EU location)
- **UI: summary row + drill-down sub-view** mirroring the AQI → Pollutants idiom. No new top-level tab, no inline multi-row card. Suppress the row entirely when no species are active.
- Drill-in sub-view shows all six species; OffSeason rows are **dimmed but visible** (not hidden) so first-time users understand the full landscape
- Pollen attribution `Data: CAMS / Copernicus · Europe only` displayed in the sub-view footer

## Spikes

| # | Name | Type | Validates | Verdict | Tags |
|---|------|------|-----------|---------|------|
| 001 | live-fetch-shape | standard | EU vs non-EU contract holds against live API with realistic May values | ✓ VALIDATED | api, network, contract |
| 002 | severity-scale | standard | Public-health-grounded grains/m³ → category mapping per species | ✓ VALIDATED | ux, taxonomy, research |
| 003 | ui-placement | standard | Best COSMIC-native placement among inline, drill-down, and seasonal-card | ✓ VALIDATED (B) | ui, figma, design |
| 004 | coverage-edges | standard | weathervane's `Region::Europe` bbox matches CAMS coverage | ✗ INVALIDATED — see correction | api, region, edge-cases |
