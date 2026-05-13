---
spike: 002
name: severity-scale
type: standard
validates: "Given grains/m³ readings, when categorized using a public-health-grounded scale, then Low/Moderate/High/Very High labels match published thresholds per species"
verdict: VALIDATED
related: [001]
tags: [ux, taxonomy, research]
---

# Spike 002: severity-scale

## What This Validates

Given grains/m³ readings from `fetch_pollen`, when categorized using the European Aeroallergen Network (EAN) scale, then Low / Moderate / High / Very High labels match published thresholds per species and correctly partition Spike 001's live fixtures.

## Research

**Open-Meteo / CAMS does not publish a user-facing severity scale.** They expose the raw grains/m³ values and expect downstream apps to bring their own taxonomy. The canonical one for the EU is the EAN (European Aeroallergen Network), which CAMS collaborates with directly via Vienna's University of Medicine. This is the right reference for an EU-only feature using CAMS data.

**EAN thresholds (grains/m³):**

| Family | Species | Low | Moderate | High | Very High |
|--------|---------|-----|----------|------|-----------|
| Tree | alder, birch | ≤10 | ≤100 | ≤1000 | >1000 |
| Grass/Weed | grass, mugwort, ragweed | ≤5 | ≤20 | ≤50 | >50 |
| Olive | olive | ≤10 | ≤50 | ≤200 | >200 |

Three threshold families, six species. Olive gets its own family because olive trees shed at intermediate volumes between tree pollens and grass/weed pollens. Mugwort and ragweed cluster with grass because they are weeds with similar shed dynamics.

Sources surveyed:
- EAN published scale (referenced via Springer / CAMS collaboration docs)
- AAAAI NAB scale (US-centric; uses different bands — confirmed *not* applicable here since we're locked to EAN/CAMS data)
- General health sites (Wyndly, Molekule, Northwest Asthma & Allergy) — confirm the EAN tier names but disagree on numeric bands

**Why this matters as a spike, not a coin flip:** Spike 001 found Rome's grass at 19.1 grains/m³. By the EAN grass scale that is **Moderate** (just under the 20 threshold for High). By a generic 0-12 wire-service scale, 19.1 would be off the chart at "Very High." Choosing the wrong scale would call routine spring grass "Very High" every day in Italy and ruin the signal. EAN is the right one because (a) it's the scale CAMS data is calibrated to, (b) it's species-specific, and (c) it's the published reference for the geography weathervane's pollen data actually covers.

## How to Run

```bash
rustc categorize.rs -o /tmp/cat && /tmp/cat
```

Standalone single-file program. No build system, no dependencies.

## What to Expect

Thirteen `ok` lines covering:
- The four Spike 001 fixtures (Rome grass, Berlin grass, Paris grass, Rome olive) categorized into their EAN bucket
- One off-season case (Birch @ 0.0)
- Eight boundary-condition checks at each tier transition for each scale family

Asserts internally; nonzero exit status means a categorization moved relative to EAN.

## Results — Verdict: VALIDATED

All thirteen cases pass. The proposed `categorize(Species, f32) -> PollenLevel` function:

- Maps 0.0 → `OffSeason` for **every** species. This is critical and emerged from Spike 001: weathervane's 0.0 is a documented "not in season here," semantically distinct from "Low (1-5)". Treating 0.0 as Low would surface every species year-round and bury the signal.
- Uses the EAN species-grouped threshold table, three scale families
- Categorizes Spike 001's live readings as:
  - Rome grass 19.1 → **Moderate**
  - Berlin grass 0.7 → **Low**
  - Paris grass 0.2 → **Low**
  - Rome olive 0.6 → **Low**
  - Berlin/Paris/Rome trees + Berlin olive + everywhere mugwort/ragweed (all 0.0) → **OffSeason**

Categorization yields **at most 1-2 non-OffSeason species per location in mid-May**, matching real-world allergy experience for the test sites. The taxonomy passes the smell test.

## Investigation Trail

1. Initial web search returned mostly US-centric NAB scales and generic 0-12 wire-service scales — none aligned to CAMS data. Setting `Mod` cutoff at 5 (NAB) vs 20 (EAN) is a fourfold difference; ignoring this would have shipped a wrong feature.
2. Pivoted to EAN-specific search after spotting Springer/CAMS collaboration references. EAN turned out to be the data backbone CAMS sources from, so the scale calibration matches the wire data exactly. Locked the choice.
3. Wrote `categorize.rs` as a single-file standalone program (no `cargo new`, no deps) compiling with bare `rustc`. Keeps the spike costless to throw away.
4. Added an `OffSeason` variant on top of the four EAN tiers after Spike 001's finding that most readings are 0.0 most of the year. EAN's published table starts at "Low (1-5)" and is silent about 0 — the variant is the natural extension.
5. Ran boundary cases at each tier transition. The `<=` ladder behaves correctly at exact thresholds (`grass==5.0` is Low, `grass==5.01` would be Moderate — confirmed by `grass==50.1` landing in Very High and `birch==1000.0` landing in High).

## Signal for Spikes That Follow

- **003 ui-placement:** Show the *category* label first, raw grains/m³ as secondary (caption or hover/expand). "Grass: Moderate" reads instantly; "Grass: 19.1 grains/m³" reads as data noise. Default to surfacing only non-OffSeason species, given the "at most 1-2 active" reality.
- **Build phase:** The `PollenLevel` enum should live in `weathervane` itself, not the applet. The enum is data-derived (calibrated to the wire data CAMS provides), not UI-derived. The localization adapter (`pollen_category_to_description`) belongs in `tempest/src/weather.rs` mirroring the existing `aqi_to_description` pattern.
- **i18n keys (proposed, not yet written):**
  - `pollen-level-off-season` — "Off season"
  - `pollen-level-low` — "Low"
  - `pollen-level-moderate` — "Moderate"
  - `pollen-level-high` — "High"
  - `pollen-level-very-high` — "Very high"
  - `pollen-species-alder` / `birch` / `grass` / `mugwort` / `olive` / `ragweed`
  - `label-pollen` — for the AQI-style summary row
  - `pollen-attribution` — "Data: CAMS / Copernicus" (no license requirement currently, but attribution is good practice)
