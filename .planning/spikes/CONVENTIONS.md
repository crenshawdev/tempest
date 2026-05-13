# Spike Conventions

Patterns established across the first cosmic-ext-applet-tempest spike session (weathervane v0.4 pollen integration). New spikes follow these unless the question requires otherwise.

## Stack

- **Rust** is the project language, but spike code is throwaway — single-file `rustc` programs with no dependencies beat `cargo new` for one-off categorization or parsing reference impls.
- **`curl` against the production API** is the right tool for "does the wire contract hold" questions. Pull JSON to `raw-*.json` files and let later commands parse them.
- **Plain HTML + inline CSS** for UI placement spikes. No build step, no Tailwind, no framework. Open in a browser; compare variants side by side. The goal is to feel the layout, not to ship CSS.

## Structure

- Each spike lives in `.planning/spikes/NNN-descriptive-name/`.
- Captured network responses sit alongside the README as `raw-{site}.json`.
- Reference implementations are single-file (`categorize.rs`, `mockup.html`) that the README links to and explains. No nested directories.
- READMEs follow the gsd-spike YAML frontmatter and section structure (What This Validates / Research / How to Run / What to Expect / Results / Investigation Trail / Signal).

## Patterns

- **Probe at boundaries.** Pick coordinates that triangulate edges, not just "obviously inside" cases. Spike 001 covered EU vs non-EU; Spike 004 probed all four cardinal directions of the bbox.
- **Spike a categorization before a UI.** Raw API numbers are usually not the user-facing artifact. Until the categorization scheme is settled, UI mockups can't be trusted.
- **HTML mockups annotate themselves.** Each variant in the UI mockup carries inline pros/cons and a written verdict block. Avoids the "which screenshot did we pick again?" problem when wrapping up.
- **Suspect the obvious gate.** If a library exposes a region predicate that *looks* like it would gate a feature cleanly, probe whether that predicate is calibrated to the same data source. Spike 004 caught a mismatch between weathervane's `Region::Europe` (MeteoAlarm-calibrated) and CAMS pollen coverage.
- **Honor the library's documented contract.** When a function returns `Result<Option<T>>` with documented "outside coverage → `Ok(None)`" semantics, *that's* the gate. Don't add a redundant region check on top.

## Tools & Libraries

- `curl` — every spike that touches network.
- `rustc` standalone (no Cargo) — for single-file reference functions.
- No new dependencies added to the tempest workspace; spike scratch code stays outside the build graph.

## Things to Avoid

- Generic US/global pollen scales (AAAAI/NAB) for a CAMS-sourced EU feature — they would mis-categorize routine spring grass as "Very High" by a factor of ~4. Use the EAN scale, which is what CAMS data is calibrated to.
- Ad-hoc string parsing of Open-Meteo JSON — the response has both `current_units.grass_pollen` (string) and `current.grass_pollen` (number) with the same key. Use typed deserialization (serde) in real code; if grepping in a spike, take the second match.
- New top-level tabs in the popup. The Current / Hourly / 7-Day segmented control is the locked top-level vocabulary. New features land as summary rows in Current and drill-down sub-views.
