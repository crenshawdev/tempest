# Phase 01 — Pollen Integration (weathervane v0.4)

**Status:** ready for execution, not started
**Branch (proposed):** `pollen-integration` — confirm with user before creating
**Spike basis:** `.planning/spikes/MANIFEST.md` and the four spike READMEs (001-004)
**Findings skill:** `.claude/skills/spike-findings-cosmic-ext-applet-tempest/` (auto-loads in build sessions)
**Issue tracker:** GitLab issue #126 (Mattermost feature request)

## Why This Plan Looks Light

This is a deliberately lightweight phase file. The spike session produced a verified implementation blueprint at `.claude/skills/spike-findings-cosmic-ext-applet-tempest/references/{api-and-coverage,categorization-and-ui}.md` containing every design decision, code pattern, and constraint. The references are the spec; this file is the sequencing plan, the verification checklist, and a place to track progress when resuming across sessions.

## Locked Decisions (from spiking)

1. Bump `weathervane` 0.3 -> 0.4 in `Cargo.toml`
2. Call `fetch_pollen` unconditionally as a fourth parallel task in the existing `Task::batch`; do NOT gate on `Region::Europe` (Spike 004 invalidated that gate — CAMS covers Levant, North Africa, east Turkey)
3. Categorize with the EAN scale, five-variant `PollenLevel` enum (`OffSeason / Low / Moderate / High / VeryHigh`); 0.0 always collapses to `OffSeason`
4. UI: summary row in Current -> drill-down sub-view (mirrors AQI -> Pollutants idiom). No new top-level tab. Summary row suppressed when no coverage or all six OffSeason
5. Drill-in shows all six species; OffSeason rows dimmed (~55% opacity), not hidden
6. Footer attribution: `Data: CAMS / Copernicus`
7. New Fluent keys land in `i18n/en/cosmic_ext_applet_tempest.ftl` and `i18n/en-US/cosmic_ext_applet_tempest.ftl` only; Weblate handles other locales

## Estimate

About 110 lines additive across:
- `Cargo.toml` (1 line changed)
- `src/weather.rs` (~50 lines added: re-exports, `PollenLevel`, `PollenSpecies`, `categorize_pollen`, two localization adapters)
- `src/applet.rs` (~70 lines added: state field, three messages, fourth `Task::perform`, summary-row block in `render_current_view`, new `render_pollen_view` method)
- `i18n/en/cosmic_ext_applet_tempest.ftl` (13 keys)
- `i18n/en-US/cosmic_ext_applet_tempest.ftl` (13 keys)

No structural refactoring of `applet.rs` required. The 76 KB monolith stays a monolith for this phase.

## Execution Sequence

Each step is small enough to review and back out independently. Run `just check` (which wraps `cargo fmt --check` and `cargo clippy`) after every code-touching step. Pause for user verification at the marked checkpoints.

### Step 1 — Branch

User confirms branch name. Default proposal: `pollen-integration`. Create from current `main`:

    git checkout main && git pull && git checkout -b pollen-integration

Do not proceed past step 1 until the user confirms.

### Step 2 — Bump weathervane

- Edit `Cargo.toml`: change `weathervane = "0.3"` to `weathervane = "0.4"`
- Run `cargo update -p weathervane` (or `cargo generate-lockfile`) to refresh `Cargo.lock`
- Run `just check` to confirm nothing in the existing surface broke (the 0.3 -> 0.4 release is additive — pollen module added, prior APIs unchanged)

### Step 3 — Extend `src/weather.rs` re-exports

Add `fetch_pollen` and `PollenData` to the existing `pub use weathervane::{...};` block. Verify `just check` passes.

### Step 4 — Categorization layer

Add to `src/weather.rs`:

- `PollenLevel` enum (5 variants, `derive(Debug, Clone, Copy, PartialEq, Eq)`)
- `PollenSpecies` enum (6 variants, `derive(Debug, Clone, Copy)`) — re-export-style; matches the field names of `weathervane::PollenData`
- `categorize_pollen(species: PollenSpecies, grains: f32) -> PollenLevel` — EAN thresholds, 0.0 collapses to `OffSeason`. Implementation in `.claude/skills/spike-findings-cosmic-ext-applet-tempest/references/categorization-and-ui.md`. Reference asserts in `.planning/spikes/002-severity-scale/categorize.rs`.

Add unit tests in `#[cfg(test)] mod tests` covering Spike 001 fixtures (Rome 19.1, Berlin 0.7, Paris 0.2, Rome olive 0.6) plus boundary cases at each EAN tier transition for each scale family. Verify `cargo test` passes.

### Step 4a — Pause point

Categorization function exists in isolation, tested. No UI yet, no `applet.rs` touched. Good handoff point if the user wants to inspect the function shape before any state-shape changes. **Stop and surface the diff for review.**

### Step 5 — Localization adapters

Add to `src/weather.rs` mirroring the existing `aqi_to_description` pattern:

- `pollen_level_to_description(level: PollenLevel) -> String`
- `pollen_species_to_description(species: PollenSpecies) -> String`

Both match on the enum and return `crate::fl!(...)`. The Fluent keys don't exist yet — adding them is the next step, so this code won't compile until step 6 lands.

### Step 6 — Fluent keys

Append to **both** `i18n/en/cosmic_ext_applet_tempest.ftl` and `i18n/en-US/cosmic_ext_applet_tempest.ftl`. Mirror what is already in the file: the AQI keys cluster after the pollutant labels around line 80. Place pollen keys after the AQI block.

Keys to add (13 total):

    label-pollen = Pollen
    pollen-attribution = Data: CAMS / Copernicus

    pollen-level-off-season = Off season
    pollen-level-low = Low
    pollen-level-moderate = Moderate
    pollen-level-high = High
    pollen-level-very-high = Very high

    pollen-species-alder = Alder
    pollen-species-birch = Birch
    pollen-species-grass = Grass
    pollen-species-mugwort = Mugwort
    pollen-species-olive = Olive
    pollen-species-ragweed = Ragweed

After step 6, the project should compile again. Run `just check`.

### Step 7 — Applet state and messages

**Pause first.** The user has flagged `applet.rs` as the 76 KB monolith requiring scope confirmation before structural edits. This step is additive, not structural — but show the planned diff before applying.

Then add to `Tempest` struct:

    /// Pollen data, when covered. Outer Option distinguishes "not yet fetched"
    /// from inner Option's "fetched but uncovered" (Ok(None) from API).
    pollen: Option<Option<PollenData>>,
    /// Whether the pollen drill-down sub-view is currently displayed.
    showing_pollen: bool,

Add to `Default for Tempest`:

    pollen: None,
    showing_pollen: false,

Add to `Message` enum:

    PollenUpdated(Result<Option<PollenData>, String>),
    ShowPollen,
    HidePollen,

Wire `PollenData` import in `applet.rs:18`. Verify `just check`.

### Step 8 — Parallel fetch

In `update(Message::Refresh)` at `applet.rs:702-732`, add the fourth parallel task and extend the `Task::batch`:

    let pollen_task = Task::perform(
        async move { fetch_pollen(lat, lon).await.map_err(|e| e.to_string()) },
        |result| Action::App(Message::PollenUpdated(result)),
    );

    return Task::batch([weather_task, air_quality_task, alerts_task, pollen_task]);

### Step 9 — Message handlers

Add the three message arms. `PollenUpdated` collapses errors to `Some(None)` (treat fetch failures as "no data," don't surface them):

    Message::PollenUpdated(result) => {
        match result {
            Ok(data) => self.pollen = Some(data),
            Err(e) => {
                self.pollen = Some(None);
                tracing::warn!("pollen fetch failed: {e}");
            }
        }
    }
    Message::ShowPollen => self.showing_pollen = true,
    Message::HidePollen => self.showing_pollen = false,

### Step 10 — Summary row in `render_current_view`

Locate the existing AQI summary row (`applet.rs:1262-1277`). Add the pollen row beneath it, with the same `button::custom` -> `Message::ShowPollen` shape.

Suppression logic (suppress the row entirely if):
- `pollen` is `None` (not fetched yet)
- `pollen` is `Some(None)` (no CAMS coverage)
- All six species categorize to `OffSeason`

When at least one species is active, render:
- Headline: `"{species} {level}"` of the highest-severity active species (sort active descending by `PollenLevel`)
- Caption: `"Pollen · {n} other active"` when more than one is active; just `"Pollen"` otherwise

### Step 11 — `render_pollen_view`

New method mirroring `render_pollutants_view` (`applet.rs:1303-1361`). Closely model the existing function's layout. Show all six species; OffSeason rows dimmed.

Wire the dispatch in the main view function: when `self.showing_pollen` is true, render `render_pollen_view` instead of the normal popup body, mirroring how `showing_pollutants` is handled today.

### Step 11a — Stop and verify

Build and install locally. Trigger refresh with three coordinate scenarios:

| Scenario | Location | Expected |
|----------|----------|----------|
| EU active species | Berlin, Helsinki, Rome, Paris | Summary row visible with grass + possibly olive; drill-in shows all six with OffSeason dimmed |
| EU no active species (synthetic) | Any EU coord in winter | Summary row suppressed; drill-in inaccessible |
| Non-EU (no coverage) | New York, Tokyo, Sydney | Summary row suppressed; no errors |
| Fetch failure | Disconnect network | Summary row suppressed silently; no error toast |

**Stop here. Await user verification before continuing past this point.**

### Step 12 — Final lint and commit

After UAT passes:

- `just fmt`
- `just check` (full lint pass, zero warnings)
- `cargo test`
- Draft a commit message for user approval. Suggested subject: `add pollen support via weathervane 0.4`
- Do NOT commit without explicit approval. Do NOT push to GitLab without explicit approval.

## Verification Checklist

Tick when complete.

- [ ] Branch created and confirmed
- [ ] `Cargo.toml` and `Cargo.lock` reflect weathervane 0.4
- [ ] `src/weather.rs` re-exports `fetch_pollen`, `PollenData`
- [ ] `PollenLevel` and `PollenSpecies` enums defined
- [ ] `categorize_pollen` with EAN thresholds + unit tests passing
- [ ] Localization adapters in `src/weather.rs`
- [ ] 13 Fluent keys in both `en/` and `en-US/`
- [ ] State fields and messages added
- [ ] Fourth parallel `Task::perform` wired
- [ ] Three new message handlers
- [ ] Summary row in `render_current_view` with suppression rules
- [ ] `render_pollen_view` with dimmed OffSeason rows
- [ ] `just check` passes with zero warnings
- [ ] Manual UAT: Berlin / Rome / NYC / network-off scenarios all behave as described in Step 11a
- [ ] User-approved commit message

## Known Follow-Ups (Out of Scope for This Phase)

- Upstreaming `categorize_pollen` into `weathervane` itself (data-derived, not UI-derived). Open question; not blocking this phase.
- Updating `weathervane/API.md` with the missing pollen section (the 0.4 release shipped without docs). Separate housekeeping task in the weathervane repo, not this one.
- Hourly / 7-day pollen forecast. Open-Meteo's pollen endpoint supports `hourly` variables; the spike used `current` only. Out of scope for the first cut; revisit if users ask.

## Resume Hints

If picking this back up after a context reset:

1. Check `git status` and the verification checklist above to see how far the work got
2. Re-read `.claude/skills/spike-findings-cosmic-ext-applet-tempest/SKILL.md` (the implementation blueprint)
3. Re-read this file
4. Continue from the first unchecked checklist item
