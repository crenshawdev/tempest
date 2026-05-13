---
spike: 003
name: ui-placement
type: standard
validates: "Given the COSMIC Figma design vocabulary, when three UI variants are mocked, then one approach reads as native COSMIC and survives the 0.0-during-off-season case"
verdict: VALIDATED
related: [001, 002]
tags: [ui, figma, design]
---

# Spike 003: ui-placement

## What This Validates

Given the COSMIC Figma design vocabulary already in use in tempest (480px dark popup, segmented Current/Hourly/7-Day tabs, AQI summary-row → Pollutants drill-down pattern, divider::horizontal section breaks), when three placement variants for pollen are mocked side-by-side, then one approach reads as native COSMIC and survives the "1-2 active species, 4-5 OffSeason" reality surfaced by Spike 001.

## Research

Source for COSMIC vocabulary:
- `src/tempest-alerts.png` — the in-repo Figma reference showing the live popup (header with Updated/refresh/alert/settings, large blue location link, Current/Hourly/7-Day segmented tabs, divider-separated content sections, info cards with severity colors)
- `applet.rs:1248-1300` — existing AQI summary-row pattern: `button::custom` containing a 20pt headline + caption, with `go-next-symbolic` chevron, classed as `Button::Text`, dispatching `Message::ShowPollutants`
- `applet.rs:1303-1371` — existing `render_pollutants_view` sub-view: back button (`Button::Link`), centered heading, `list_column` of label-value rows, container padding `space_m`

Three variants in play:
- **A: Inline summary row.** One row, AQI-style. "Grass Moderate" headline, "Pollen · 1 other active" caption, chevron → drill-in.
- **B: Drill-down sub-view (drill-in shows all six species).** Two-step: summary row in Current → full sub-view with severity pills + off-season rows dimmed.
- **C: Seasonal active-species card.** Inline expansion in Current showing only non-OffSeason species; no drill-down.

## How to Run

```bash
xdg-open .planning/spikes/003-ui-placement/mockup.html
# or open in any browser
```

## What to Expect

A scrollable HTML page with all three variants rendered at the actual 480px popup width against a dark background. Each variant is annotated with pros, cons, and trade-offs. The page ends with a written verdict block.

## Results — Verdict: VALIDATED → **Variant B (drill-down) chosen**

Variant B wins on four counts:

1. **Symmetry with AQI.** The existing applet has one drill-down idiom: summary row in Current → sub-view with all detail. Pollen using the same idiom means zero new vocabulary to learn and zero new code patterns to maintain. Variants A and C either truncate or break this symmetry.
2. **Off-season legibility.** Variant B is the only one with room to show the full six species. Dimmed off-season rows (~55% opacity, "Off season" caption instead of a number) tell the user "we know about birch and alder; they're just not in season here right now." Hiding them entirely (Variant C) would read as "the app forgot a feature" on first encounter.
3. **Extreme handling.** With 1 active species, Variants A and B look identical at the summary level. With 4 active species (plausible in southern Europe late June), Variant C stretches the Current tab past the alerts/settings cutoff on smaller screens; Variant B doesn't grow.
4. **Attribution placement.** "Data: CAMS / Copernicus · Europe only" lands naturally as a sub-view footer, where it can be read without taking real estate from the more glance-able Current tab.

**Summary-row content rule (from the mockup):** show the *highest-severity active species* and its category in the headline; caption counts the others. Examples:
- `Grass Moderate · 1 other active`
- `Birch High`  (no caption when only one active)
- (row entirely absent when zero species active)

**Suppressing the row entirely when nothing is active** is a deliberate choice: "Off season" as a top-level state is information without action — better to give the space back. Same principle as suppressing the alerts panel when there are no alerts.

## Investigation Trail

1. Sketched Variant A first since the existing AQI pattern is the local strong default. Quickly realized "Grass Moderate" with a generic chevron loses the "1 other active" signal unless the caption carries it explicitly.
2. Sketched Variant C next to see if an inline expansion could replace the drill-down entirely. The inline option looked clean with 2 active species (the Rome fixture) but breaks the symmetry with AQI. Designs that diverge "because they can" become tech debt — pulled this lever and rejected the variant.
3. Variant B emerged as the safest re-use of existing primitives. The unanswered question is whether OffSeason rows belong in the sub-view at all — settled on "yes, dimmed" because removing them creates a "did the app drop a feature?" moment for first-time users.
4. Did not mock a tab variant (full-tab between Hourly and 7-Day). The user's MANIFEST requirement explicitly forbids a new top-level tab — that constraint pre-empts the option.
5. Stopped before pixel-tuning. Exact pill colors, icon vs. text, and Fluent string wording belong in the build phase, not the spike — locking them prematurely would constrain the implementation without evidence.

## Signal for Spikes That Follow / Build Phase

- **State machine:** add a `showing_pollen: bool` flag mirroring `showing_pollutants`. Don't share state — two independent drill-downs with parallel back buttons is the cleanest mental model.
- **Message type:** `Message::ShowPollen` / `Message::HidePollen`.
- **View function:** `render_pollen_view` mirroring `render_pollutants_view`. ~70 lines of additive code; no `applet.rs` restructuring.
- **Summary-row gating:** if `pollen.is_none()` (non-EU) → no row. If all six species are `OffSeason` → no row. If 1+ active → one row.
- **Severity pill color mapping (deferred to build):** Low green-ish, Moderate amber, High orange-red, Very High magenta. Mockup used arbitrary colors; the real palette should pull from `cosmic::theme` accents, not hardcoded hex.
