---
spike: 004
name: coverage-edges
type: standard
validates: "Given detect_region(...) == Region::Europe, when called against UK/Turkey/Iceland/Cyprus/Moscow/Reykjavik, then weathervane's region bounding box agrees with CAMS coverage"
verdict: INVALIDATED
related: [001]
tags: [api, region, edge-cases]
---

# Spike 004: coverage-edges

## What This Validates

Given `weathervane::detect_region(lat, lon) == Region::Europe`, when called against twelve coordinates around the bounding box edge, then weathervane's region check agrees with the actual CAMS pollen-coverage boundary (i.e., is a safe gate for whether to attempt `fetch_pollen`).

**Result: it does not agree. Hypothesis invalidated, with a critical implication for the build.**

## Research

`weathervane::geo::is_europe_bounds` (geo.rs:75-78):

```rust
fn is_europe_bounds(lat: f64, lon: f64) -> bool {
    (35.0..=71.0).contains(&lat) && (-25.0..=40.0).contains(&lon)
}
```

Documented purpose: *"European countries covered by **MeteoAlarm**. Uses European AQI."* It was calibrated for weather-alert routing and AQI scale selection, not pollen. CAMS European Air Quality Forecast publishes a wider domain — roughly 30°N–72°N, -25°E–45°E — that bleeds into North Africa and the Levant.

## How to Run

```bash
for entry in reykjavik:64.13:-21.94 helsinki:60.17:24.94 london:51.50:-0.12 \
             lisbon:38.72:-9.14 cyprus:35.16:33.36 istanbul:41.01:28.98 \
             ankara:39.93:32.85 erzurum:39.91:41.27 telaviv:32.08:34.78 \
             moscow:55.75:37.62 yekaterinburg:56.83:60.59 cairo:30.04:31.24; do
  name=${entry%%:*}; rest=${entry#*:}; lat=${rest%:*}; lon=${rest#*:}
  curl -sS "https://air-quality-api.open-meteo.com/v1/air-quality?latitude=${lat}&longitude=${lon}&current=grass_pollen&timezone=auto" > "raw-${name}.json"
done
```

Raw JSON for all twelve probes captured in `raw-*.json` alongside this README.

## What to Expect

For each site, three signals to compare:
1. Whether `is_europe_bounds(lat, lon)` returns true
2. Whether Open-Meteo returns a numeric `grass_pollen` value or `null`
3. Whether the two agree

## Results — Verdict: INVALIDATED (with critical correction for the build)

| Site | lat | lon | wv `Region::Europe`? | CAMS returns | Agreement |
|------|-----|-----|----------------------|--------------|-----------|
| Reykjavik | 64.13 | -21.94 | ✓ | 0.0 (numeric) | ✓ |
| Helsinki | 60.17 | 24.94 | ✓ | 0.0 | ✓ |
| London | 51.50 | -0.12 | ✓ | 0.7 | ✓ |
| Lisbon | 38.72 | -9.14 | ✓ | **24.2** | ✓ |
| Cyprus | 35.16 | 33.36 | ✓ (barely) | 3.7 | ✓ |
| Istanbul | 41.01 | 28.98 | ✓ | 6.3 | ✓ |
| Ankara | 39.93 | 32.85 | ✓ | 9.8 | ✓ |
| Moscow | 55.75 | 37.62 | ✓ (lon 37.62 < 40) | 1.5 | ✓ |
| **Erzurum** | 39.91 | 41.27 | **✗** (lon > 40) | **9.0** | **MISMATCH** |
| **Tel Aviv** | 32.08 | 34.78 | **✗** (lat < 35) | **0.9** | **MISMATCH** |
| **Cairo** | 30.04 | 31.24 | **✗** (lat < 35) | **2.2** | **MISMATCH** |
| Yekaterinburg | 56.83 | 60.59 | ✗ | null | ✓ |

**Three of twelve disagree, all in the same direction:** CAMS pollen coverage is *wider* than weathervane's `Region::Europe`. It extends south into the Levant and North Africa, and east into eastern Turkey. The east boundary lies between Erzurum (41.27°E, covered) and Yekaterinburg (60.59°E, not covered).

**Critical implication:** If the build gates pollen on `detect_region(...) == Region::Europe` — the obvious-looking gate — it will **incorrectly hide pollen data for Tel Aviv, Beirut, Cairo, eastern Turkey, and the surrounding Levant**, even though the API has data for those locations. The gate would manufacture a coverage gap that the API doesn't actually have.

**Correction for the build:**

> Do **not** gate the pollen call on region. Just call `fetch_pollen(lat, lon)` unconditionally and let `Ok(None)` be the gate.

This is also what the weathervane API was designed for. The pollen module's doc comment is explicit:

> "Calls outside Europe return `Ok(None)` rather than an error so callers can treat pollen as a region-optional feature the same way they already do for alerts."

The right pattern mirrors `fetch_alerts`, which also returns "empty for unsupported regions" — the applet calls it for everyone and shows nothing when the result is empty.

## Investigation Trail

1. Picked twelve sites to triangulate boundaries in all four directions plus a guaranteed-outside sanity check (Yekaterinburg). Hypothesis going in was that `Region::Europe` and CAMS coverage would mostly align with maybe a small fringe discrepancy in the Levant.
2. First-round `grep` extracted the `current_units` `"grains/m³"` string by mistake (it precedes the value in the JSON). Corrected to take the second match per file, which is the actual value under `current`. Worth flagging because the same trap exists in real parsing — weathervane uses serde with typed fields, so it's safe, but ad-hoc string parsing of this endpoint would hit it.
3. Lisbon at 24.2 grass/m³ was a striking high-end fixture — by the EAN grass scale that's "High" (>20 cutoff), confirming southern Europe in mid-May is genuinely in grass-peak territory. Validates that the categorization spike's thresholds match real-world signal.
4. Three mismatches all in one direction (CAMS wider than weathervane bbox) made the conclusion immediate: don't use the gate. Confirmed by reading the weathervane pollen module's own doc comment which explicitly anticipates this pattern.
5. Yekaterinburg's `null` confirms CAMS does have an east boundary; the exact line lies somewhere between 41.27°E (Erzurum, covered) and 60.59°E (Yekaterinburg, not). Not worth pinning down further — the build pattern doesn't depend on knowing the exact boundary.

## Signal for the Build

- **Fetch unconditionally.** Add a fourth parallel task to the `Task::batch` at `applet.rs:702`:

```rust
let pollen_task = Task::perform(
    async move { fetch_pollen(lat, lon).await.map_err(|e| e.to_string()) },
    |result| Action::App(Message::PollenUpdated(result)),
);
return Task::batch([weather_task, air_quality_task, alerts_task, pollen_task]);
```

- **Gate at render time, not fetch time.** State carries `pollen: Option<Option<PollenData>>` (outer Option = "have we fetched yet"; inner Option = "did API return data"). Summary row renders only when inner is `Some` and at least one species is non-`OffSeason`.
- **No region detection involved in pollen logic at all.** The `Region` enum is the wrong abstraction for this feature.
- **Document the coverage:** the attribution caption in the drill-down sub-view should say `"Data: CAMS / Copernicus"` rather than `"Europe only"` — coverage is *de facto* CAMS European model domain, which is wider than people would assume from the name.
