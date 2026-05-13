# Spike Wrap-Up Summary

**Date:** 2026-05-13
**Spikes processed:** 4
**Feature areas:** API + Coverage Gating; Categorization + UI Placement
**Skill output:** `./.claude/skills/spike-findings-cosmic-ext-applet-tempest/`

## Processed Spikes

| # | Name | Type | Verdict | Feature Area |
|---|------|------|---------|--------------|
| 001 | live-fetch-shape | standard | ✓ VALIDATED | API + Coverage Gating |
| 002 | severity-scale | standard | ✓ VALIDATED | Categorization + UI Placement |
| 003 | ui-placement | standard | ✓ VALIDATED (Variant B) | Categorization + UI Placement |
| 004 | coverage-edges | standard | ✗ INVALIDATED — correction surfaced | API + Coverage Gating |

## Key Findings

- **Wire contract holds.** weathervane v0.4's `fetch_pollen` returns `Some(PollenData)` for CAMS-covered coordinates and `None` for everywhere else. JSON shape matches the parser exactly.
- **Most species are 0.0 most of the year per location.** Mid-May 2026 real data showed only 1-2 species active per EU site. UI must treat 0.0 as a distinct `OffSeason` state.
- **EAN is the right scale, not AAAAI/NAB.** A 4× threshold difference on grass; ignoring this would call routine spring grass "Very High" every day in Italy.
- **Do NOT gate pollen on `Region::Europe`.** CAMS coverage is wider than weathervane's MeteoAlarm-calibrated bbox — Tel Aviv, Cairo, and east Turkey all return real numeric data despite being outside `Region::Europe`. Call unconditionally and use `Ok(None)` as the gate.
- **UI lands cleanly in the existing AQI → Pollutants drill-down pattern.** ~70 lines additive in `applet.rs`, no restructuring of the 76 KB monolith required.
- **weathervane's `API.md` is stale** — no pollen section despite v0.4 release. Worth flagging upstream as a housekeeping note (not part of this work).

## Implementation Estimate (signal for the next phase)

- ~10 lines: `Cargo.toml` bump + `src/weather.rs` re-exports + localization adapters
- ~30 lines: `categorize_pollen` function (could live upstream in `weathervane` instead)
- ~70 lines: `applet.rs` — fourth `Task::batch` entry, state field, three messages, summary-row block in `render_current_view`, `render_pollen_view` sub-view
- 13 new Fluent keys in `i18n/en/` and `i18n/en-US/` only

No structural refactoring of `applet.rs` needed. No new external dependencies. No upstream weathervane changes required for this iteration (categorization upstream is a future option, not a blocker).
