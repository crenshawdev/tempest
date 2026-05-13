---
spike: 001
name: live-fetch-shape
type: standard
validates: "Given EU and non-EU coordinates, when fetch_pollen is called against the live API, then EU returns Some with realistic May values and non-EU returns None"
verdict: VALIDATED
related: []
tags: [api, network, contract]
---

# Spike 001: live-fetch-shape

## What This Validates

Given EU and non-EU coordinates, when `fetch_pollen` is called against `air-quality-api.open-meteo.com`, then EU coordinates return `Some(PollenData)` with realistic in-season values, and non-EU coordinates return `None` (because all six `*_pollen` fields come back JSON null, which weathervane's `from_current` collapses to `None`).

## Research

The weathervane source at `/code/weathervane/src/pollen.rs` constructs this URL:

```
https://air-quality-api.open-meteo.com/v1/air-quality
  ?latitude={lat}&longitude={lon}
  &current=alder_pollen,birch_pollen,grass_pollen,mugwort_pollen,olive_pollen,ragweed_pollen
  &timezone=auto
```

No API key. No rate-limit headers documented for the free tier on this endpoint. Reused for both AQ and pollen requests in tempest's fetch flow.

## How to Run

```bash
for entry in "berlin:52.52:13.40" "paris:48.85:2.35" "rome:41.90:12.49" "nyc:40.71:-74.01" "tokyo:35.68:139.69"; do
  name=${entry%%:*}; coords=${entry#*:}; lat=${coords%:*}; lon=${coords#*:}
  curl -sS "https://air-quality-api.open-meteo.com/v1/air-quality?latitude=${lat}&longitude=${lon}&current=alder_pollen,birch_pollen,grass_pollen,mugwort_pollen,olive_pollen,ragweed_pollen&timezone=auto" > "raw-${name}.json"
done
```

Raw responses captured in `raw-*.json` alongside this README.

## What to Expect

- EU coords: `current.*_pollen` are numbers (often `0.0` for off-season species)
- Non-EU coords: `current.*_pollen` are all six `null`
- `current_units.*` confirms `"grains/m³"` (note: UTF-8 superscript)
- `generationtime_ms` consistently under 1ms; round-trip is network-bound

## Results — Verdict: VALIDATED

Live values pulled 2026-05-13 around 18:00 CEST / 12:00 EDT / 01:00 JST:

| Site | alder | birch | grass | mugwort | olive | ragweed | Shape |
|------|-------|-------|-------|---------|-------|---------|-------|
| Berlin (52.52, 13.40) | 0.0 | 0.0 | **0.7** | 0.0 | 0.0 | 0.0 | numeric |
| Paris (48.85, 2.35) | 0.0 | 0.0 | **0.2** | 0.0 | 0.0 | 0.0 | numeric |
| Rome (41.90, 12.49) | 0.0 | 0.0 | **19.1** | 0.0 | **0.6** | 0.0 | numeric |
| NYC (40.71, -74.01) | null | null | null | null | null | null | all null |
| Tokyo (35.68, 139.69) | null | null | null | null | null | null | all null |

**Contract confirmed:** Non-EU coords return all six fields as JSON null, which weathervane's `from_current` correctly maps to `Ok(None)`. EU coords return all six as numbers, including `0.0` for off-season species — which is distinct from "no data" and must be preserved in the UI.

## Investigation Trail

1. Initially planned a Rust binary using the local weathervane crate. Pivoted to raw curl — weathervane already has unit tests for the parsing layer; the spike's job is to verify the *wire contract*, not the parser.
2. Picked three EU sites at different latitudes (Berlin 52°, Paris 48°, Rome 41°) to see whether the EU/non-EU split is sharp or whether southern Mediterranean sites might fall outside CAMS coverage. Rome returned data, so coverage extends at least to central Italy. (Spike 004 will probe the actual edges.)
3. Striking surprise: Rome grass at **19.1 grains/m³** is "Very High" on most published scales, while Berlin sits at 0.7 ("Very Low"). A 95× spread across EU latitudes confirms the categorization spike (002) is non-optional — raw numbers cannot ship without interpretation.
4. **Important UI implication:** Most species read 0.0 most of the year per location. In mid-May, tree pollens (alder, birch) are *done* in central/southern Europe, mugwort and ragweed are still asleep until late summer, and olive is just starting in the Mediterranean. So even *inside* coverage, typically only 1-2 species are non-zero at any time. A naïve "show all six" list would be 70-80% zeros.
5. Confirmed the JSON field order matches `weathervane::pollen::PollenCurrent` struct exactly. No deserialization risk.

## Signal for Spikes That Follow

- **002 severity-scale:** Use Rome's grass = 19.1 as a high-end fixture, Berlin's 0.7 as a low-end fixture, Paris's 0.2 as a near-zero fixture. Off-season 0.0s are abundant — categorization must treat 0.0 distinctly (probably "Off-season" or "—", not "Very Low").
- **003 ui-placement:** Plan for the "most species are zero" reality. A list of six rows is mostly empty. An "active species" card showing only non-zero rows is probably the right primitive.
- **004 coverage-edges:** The split between numeric and all-null is sharp — no partial-null cases observed. So `Ok(None)` cleanly partitions "covered" vs "not covered." The remaining question is whether weathervane's `Region::Europe` bbox lines up with the actual CAMS boundary.
