# Tempest 2.8.3 COSMIC Conformance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring `src/applet.rs` into compliance with the libcosmic and cosmic-style rule sets across three axes — typography helpers, scrollable padding, and sub-view header consistency — and ship as 2.8.3.

**Architecture:** Five sequential commits on `cosmic-conformance-2-8-3`, each isolating one mechanical or structural change so any commit can be reverted cleanly. Spec at `docs/superpowers/specs/2026-05-14-cosmic-conformance-2-8-3-design.md`.

**Tech Stack:** Rust, libcosmic 1.0.0 (specific git commit), iced. Build via `just check` and `just install-dev`. No unit tests for UI surfaces — verification is build success plus manual popup inspection.

**Conventions:**
- `spacing` in every render fn is `cosmic::theme::spacing()`. Reuse that local; do not re-look-up.
- `widget::text::*` helpers in libcosmic accept `impl Into<Cow<'a, str>>`, same as raw `text(...)`.
- Run `just check` after every code commit; this runs `cargo fmt --check` and `cargo clippy -- -D warnings`. Fix every clippy warning before moving on.

---

## File Structure

| File | Role | Touched by |
|---|---|---|
| `src/applet.rs` | UI monolith. All four code commits live here. | Tasks 1–4 |
| `i18n/en/cosmic_ext_applet_tempest.ftl` | English source strings. Single key rename. | Task 4 |
| `i18n/en-US/cosmic_ext_applet_tempest.ftl` | en-US mirror. Same single key rename. | Task 4 |
| `Cargo.toml` | Version source of truth. | Task 5 |
| `Cargo.lock` | Regenerated. | Task 5 |
| `res/com.vintagetechie.CosmicExtAppletTempest.metainfo.xml` | AppData release entry. | Task 5 |
| `CHANGELOG.md` | Release entry + bottom link. | Task 5 |
| `README.md` | Mirror of changelog excerpt. | Task 5 |

Other locale .ftl files are intentionally not touched — the project's Weblate hook blocks edits to non-`en` locales (see `MEMORY.md`). Orphaned `locations-back` keys in other locales will be cleaned up by Weblate on its next sync.

---

## Task 1: Swap hand-rolled text sizes for cosmic role helpers

**Why:** `text(...).size(N)` does not track the COSMIC type scale; the role helpers (`text::title1/title4/heading/body/caption`) do. The 36/20/18/16/13/11 sites all land on or near a role boundary.

**Files:**
- Modify: `src/applet.rs` (eleven sites listed below)

- [ ] **Step 1.1: Apply the typography edits**

The pattern at every site: replace `text(EXPR).size(N)` with the matching role helper. Preserve any `.class(...)` chain.

Site-by-site mapping. Line numbers reference the current `main` snapshot; resolve drift by matching the surrounding code.

| Line | Current | Replacement | Notes |
|---|---|---|---|
| 619 | `text(&self.config.location_name).size(18)` | `widget::text::title4(&self.config.location_name)` | Location header (multi-location button branch) |
| 630 | `text(&self.config.location_name).size(18)` | `widget::text::title4(&self.config.location_name)` | Location header (single-location branch) |
| 645 | `text(crate::fl!("failed-to-load")).size(18)` | `widget::text::title4(crate::fl!("failed-to-load"))` | Error state title |
| 662 | `text(crate::fl!("loading")).size(18)` | `widget::text::title4(crate::fl!("loading"))` | Loading state title |
| 1251 | `text(...).size(36)` | `widget::text::title1(...)` | Headline temperature (35 px is the exact title1 size) |
| 1328 | `text(format!("{} {}", aq.aqi, aqi_description)).size(20)` | `widget::text::title4(format!("{} {}", aq.aqi, aqi_description))` | AQI headline |
| 1385 | `text(headline).size(20)` | `widget::text::title4(headline)` | Pollen headline |
| 1611 | `text(crate::fl!("no-active-alerts")).size(16)` | `widget::text::title4(crate::fl!("no-active-alerts"))` | Empty alerts title |
| 1698 | `text(format!("{}%", hour.precipitation_probability)).size(11)` | `widget::text::caption(format!("{}%", hour.precipitation_probability))` | Hourly precip caption |
| 1833 | `text(crate::fl!("detected-via-ip")).size(11).class(cosmic::theme::Text::Accent)` | `widget::text::caption(crate::fl!("detected-via-ip")).class(cosmic::theme::Text::Accent)` | Auto-detect subtitle |
| 1891 | `text(crate::fl!("manually-selected")).size(11).class(cosmic::theme::Text::Accent)` | `widget::text::caption(crate::fl!("manually-selected")).class(cosmic::theme::Text::Accent)` | Manual-select subtitle |
| 1954 | `text(crate::fl!("settings-auto-units-hint")).size(11)` | `widget::text::caption(crate::fl!("settings-auto-units-hint"))` | Auto-units hint |
| 1969 | `text(crate::fl!("settings-min")).size(13)` | `widget::text::body(crate::fl!("settings-min"))` | Inline unit label next to a text input — body, not caption |
| 1996 | `text(crate::fl!("settings-aqicn-token-hint")).size(11).class(cosmic::theme::Text::Accent)` | `widget::text::caption(crate::fl!("settings-aqicn-token-hint")).class(cosmic::theme::Text::Accent)` | Token hint |
| 2044 | `text(format!("{} {}", crate::fl!("settings-version"), VERSION)).size(13).class(cosmic::theme::Text::Accent)` | `widget::text::caption(format!("{} {}", crate::fl!("settings-version"), VERSION)).class(cosmic::theme::Text::Accent)` | Version footer — caption, paired with a primary button |

Disambiguation rule for any new `.size(11)` or `.size(13)` site encountered during the edit: subdued metadata or accent-styled secondary text → `caption`; inline value/label paired with an input or another body-sized element → `body`.

- [ ] **Step 1.2: Verify import**

The `text` symbol comes from `cosmic::widget::{..., text}` at the top of `applet.rs`. After this commit, `text(...)` calls in the file are only the bare ones still using `.size(N)` (if any remain) or removed entirely. The import stays — `widget::text::*` is the explicit submodule path and does not depend on the unqualified `text` import.

Run:
```bash
just check
```
Expected: clean. No warnings, no fmt diff.

- [ ] **Step 1.3: Build and visually verify**

Run:
```bash
just install-dev
```
Expected: build succeeds. Restart `cosmic-panel` or log out / log in to pick up the new applet binary. Open the popup; visit each tab (Current, Hourly, Forecast, Alerts, Settings); open Pollutants and Pollen sub-views from Current; trigger the error state by toggling network. Confirm nothing renders with a visibly broken size.

- [ ] **Step 1.4: Commit**

```bash
git add src/applet.rs
git commit -m "$(cat <<'EOF'
use cosmic text role helpers in place of raw font sizes

drops hand-rolled .size(36/20/18/16/13/11) sites in favor of
widget::text::title1/title4/body/caption so the popup tracks the
cosmic typography scale and respects system theme changes.
EOF
)"
```

---

## Task 2: Pad scrollable content so the scrollbar stops clipping

**Why:** `widget::scrollable(column)` overlays its scrollbar on the right edge of the content viewport. libcosmic's `context_drawer` wraps content in a padded container before passing it to `scrollable`. Two sites in `applet.rs` skip this.

**Files:**
- Modify: `src/applet.rs:560-565, 693` (main popup scrollable)
- Modify: `src/applet.rs:1644-1651` (alert description scrollable)

- [ ] **Step 2.1: Pad the main popup scrollable**

Edit `view_window` (the outer column construction starts at line 560).

**Before (lines 560-565):**
```rust
let mut column = widget::Column::new().spacing(spacing.space_xs).padding([
    spacing.space_xs,
    spacing.space_xs,
    spacing.space_m,
    spacing.space_xs,
]);
```

**After:**
```rust
let mut column = widget::Column::new()
    .spacing(spacing.space_xs)
    .padding([spacing.space_xs, spacing.space_xs, 0, spacing.space_xs]);
```

Rationale: top stays `space_xs`, sides stay `space_xs`, bottom moves to `0` because the scrollable's wrapping container will own the bottom padding.

**Before (line 693):**
```rust
let scrollable = widget::scrollable(column).height(cosmic::iced::Length::Shrink);
```

**After:**
```rust
let padded = widget::container(column).padding([
    0,
    spacing.space_l,
    spacing.space_l,
    0,
]);
let scrollable = widget::scrollable(padded).height(cosmic::iced::Length::Shrink);
```

Rationale: zero top (the column already has `space_xs` top), `space_l` right matches libcosmic's wide-popup context-drawer padding (popup is fixed 480 px so we are above the 392 px narrow threshold), `space_l` bottom replaces the dropped column bottom, zero left (the column already pads).

- [ ] **Step 2.2: Pad the alert description scrollable**

**Before (lines 1644-1651):**
```rust
Some(
    widget::container(
        widget::scrollable(widget::text::caption(&alert.description))
            .height(cosmic::iced::Length::Shrink),
    )
    .padding([spacing.space_xxxs, 0, spacing.space_xxxs, 0])
    .max_height(160.0),
)
```

**After:**
```rust
Some(
    widget::container(
        widget::scrollable(
            widget::container(widget::text::caption(&alert.description))
                .padding([0, spacing.space_s, 0, 0]),
        )
        .height(cosmic::iced::Length::Shrink),
    )
    .padding([spacing.space_xxxs, 0, spacing.space_xxxs, 0])
    .max_height(160.0),
)
```

Rationale: the scrollable's content gets `space_s` right padding so the scrollbar does not overlay the alert description text. The outer container's vertical padding and `max_height(160.0)` stay (deliberate vertical constraint).

- [ ] **Step 2.3: Build and visually verify**

```bash
just check && just install-dev
```

Open the popup. Confirm the right edge of every tab's content has visible space between the content and the scrollbar. Trigger a long alert (or temporarily lower `max_height` in scratch) to verify the alert description scrollbar does not clip the right edge of the caption text.

- [ ] **Step 2.4: Commit**

```bash
git add src/applet.rs
git commit -m "$(cat <<'EOF'
pad scrollable content so the scrollbar stops clipping

wraps the main popup column and the alert description text in
containers with right padding before passing them to widget::scrollable,
matching libcosmic's context_drawer convention. the scrollbar no longer
overlays content on the right edge.
EOF
)"
```

---

## Task 3: Unify forecast table header and row column widths

**Why:** The header at lines 1729-1750 declares column widths inline that approximately match the data rows at lines 1753-1791. The header reserves a `Space::new().width(space_m)` for the weather-icon column while the data row inserts a bare icon with no width wrapper. Alignment drifts when icon size or `space_m` changes. The header also misses `align_y(Center)` and the row's outer padding, and uses `caption` where the libcosmic table contract calls for `heading`.

**Files:**
- Modify: `src/applet.rs:1721-1794` (entire `render_forecast_tab` function body)

- [ ] **Step 3.1: Rewrite `render_forecast_tab` with shared column constants**

Replace the function body (lines 1722-1794) with the version below. Function signature, imports, and the line numbers around the surrounding `render_*` helpers are unchanged.

```rust
    fn render_forecast_tab(&self, weather: &WeatherData) -> Element<'_, Message> {
        use cosmic::iced::{Alignment, Length};

        const COL_DAY: Length = Length::FillPortion(3);
        const COL_ICON: Length = Length::Fixed(24.0);
        const COL_HIGH: Length = Length::FillPortion(1);
        const COL_LOW: Length = Length::FillPortion(1);
        const COL_COND: Length = Length::FillPortion(2);

        let spacing = cosmic::theme::spacing();
        let mut col = widget::Column::new()
            .spacing(spacing.space_xxs)
            .padding([0, spacing.space_xxs, 0, spacing.space_m])
            .width(Length::Fill);

        // Table header
        col = col.push(
            widget::Row::new()
                .spacing(spacing.space_xxs)
                .align_y(Alignment::Center)
                .padding([0, spacing.space_xxs])
                .push(
                    widget::container(widget::text::heading(crate::fl!("forecast-day")))
                        .width(COL_DAY),
                )
                .push(widget::container(widget::Space::new()).width(COL_ICON))
                .push(
                    widget::container(widget::text::heading(crate::fl!("forecast-high")))
                        .width(COL_HIGH),
                )
                .push(
                    widget::container(widget::text::heading(crate::fl!("forecast-low")))
                        .width(COL_LOW),
                )
                .push(
                    widget::container(widget::text::heading(crate::fl!("forecast-conditions")))
                        .width(COL_COND),
                ),
        );
        col = col.push(widget::divider::horizontal::default());

        // Data rows
        for day in &weather.forecast {
            col = col.push(
                widget::Row::new()
                    .spacing(spacing.space_xxs)
                    .align_y(Alignment::Center)
                    .padding([0, spacing.space_xxs])
                    .push(
                        widget::container(widget::text::body(format_date(&day.date)))
                            .width(COL_DAY),
                    )
                    .push(
                        widget::container(
                            widget::icon::from_name(day.condition.icon_name(false))
                                .size(20)
                                .symbolic(true),
                        )
                        .width(COL_ICON)
                        .align_x(cosmic::iced::alignment::Horizontal::Center),
                    )
                    .push(
                        widget::container(widget::text::body(
                            self.config.temperature_unit.format(day.temp_max),
                        ))
                        .width(COL_HIGH),
                    )
                    .push(
                        widget::container(widget::text::body(
                            self.config.temperature_unit.format(day.temp_min),
                        ))
                        .width(COL_LOW),
                    )
                    .push(
                        widget::container(
                            widget::text::body(condition_to_description(day.condition))
                                .wrapping(cosmic::iced::widget::text::Wrapping::None)
                                .ellipsize(cosmic::iced::widget::text::Ellipsize::End(
                                    cosmic::iced::core::text::EllipsizeHeightLimit::Lines(1),
                                )),
                        )
                        .width(COL_COND),
                    ),
            );
        }

        col.into()
    }
```

Key shifts from the current code:
- `space_m` row spacing → `space_xxs` to match the table contract from the rule
- Header gains `align_y(Center)` and `padding([0, space_xxs])`
- Header cells switch `caption` → `heading`
- Header icon-column slot becomes `widget::container(Space::new()).width(COL_ICON)` not a raw `Space::new().width(space_m)`
- Data row icon gains a wrapping container with `COL_ICON` width and centered alignment
- All five column widths are declared once as `const`s and reused on both rows

- [ ] **Step 3.2: Build and visually verify**

```bash
just check && just install-dev
```

Open the popup, switch to the Forecast tab. Confirm:
- Header text reads as 14 px / 700 weight (heading role), not 11.6 px (caption)
- Header cells line up vertically with data cells: day column, icon column (empty header / icon in rows), high, low, condition
- Icon column is exactly 24 px wide; the icon centers within it
- Vertical alignment of header text and row text shares the same baseline

- [ ] **Step 3.3: Commit**

```bash
git add src/applet.rs
git commit -m "$(cat <<'EOF'
share column widths between forecast header and rows

declares COL_DAY/COL_ICON/COL_HIGH/COL_LOW/COL_COND as length constants
and applies them to every header cell and every row cell, plus matched
align_y, padding, and spacing. header cells switch caption to heading
per the libcosmic table contract. alignment is now a property of the
constants, not a coincidence.
EOF
)"
```

---

## Task 4: Align locations sub-view to close-on-right pattern

**Why:** Pollutants and Pollen sub-views use a centered-title-plus-close-on-right header; Locations uses a back-on-left. The 2.8.2 Air Quality redesign moved deliberately to the close-on-right pattern; Locations is the holdout. Additionally, all three close buttons use `go-next-symbolic` (right chevron, implies forward nav) where `window-close-symbolic` is correct.

**Files:**
- Modify: `src/applet.rs:1402-1430` (pollutants header)
- Modify: `src/applet.rs:1475-1503` (pollen header)
- Modify: `src/applet.rs:1539-1576` (locations view)
- Modify: `i18n/en/cosmic_ext_applet_tempest.ftl:39` (`locations-back` → `locations-close`)
- Modify: `i18n/en-US/cosmic_ext_applet_tempest.ftl:163` (`locations-back` → `locations-close`)

Other locale .ftl files are not touched — Weblate hook blocks edits. The orphaned `locations-back` keys in the translated locales will sync on Weblate's next pass.

- [ ] **Step 4.1: Introduce a shared sub-view header helper**

Add a private associated method on `Tempest` near the existing `section_header` helper (~line 1797). Place it just after `section_header` or near the other render helpers — wherever the file's existing convention puts shared widget helpers. Suggested placement: immediately above `render_pollutants_view`.

```rust
    /// Header for sub-views (pollutants, pollen, locations): centered title
    /// with a close button on the right.
    fn subview_header(title: String, on_close: Message) -> Element<'static, Message> {
        let spacing = cosmic::theme::spacing();
        widget::Row::new()
            .align_y(cosmic::iced::Alignment::Center)
            .spacing(spacing.space_xs)
            .push(
                widget::container(widget::text::heading(title))
                    .width(cosmic::iced::Length::Fill)
                    .align_x(cosmic::iced::alignment::Horizontal::Center),
            )
            .push(
                widget::button::icon(widget::icon::from_name("window-close-symbolic"))
                    .padding(spacing.space_xxs)
                    .on_press(on_close),
            )
            .into()
    }
```

Design notes:
- Returns `Element<'static, Message>` because the title is taken by value as `String` and the on_close `Message` is `Clone`/`'static` per the existing enum.
- Uses `widget::button::icon` (the icon-button family) with `window-close-symbolic`, replacing the current `button::custom(Row { body("Close"), go-next-symbolic })` pattern. The icon-only button is the COSMIC-native close affordance and removes the need for the `air-quality-close` and `locations-back` strings on those buttons.
- Centered title via the `Length::Fill` + `Horizontal::Center` container wrap is consistent with how Pollutants/Pollen already center their title; the close button retains intrinsic width.

- [ ] **Step 4.2: Replace the pollutants header**

**Before (lines 1411-1430):**
```rust
        let close_btn = widget::button::custom(
            widget::Row::new()
                .spacing(spacing.space_xxxs)
                .align_y(cosmic::iced::Alignment::Center)
                .push(widget::text::body(crate::fl!("air-quality-close")))
                .push(widget::icon::from_name("go-next-symbolic").size(16)),
        )
        .class(cosmic::theme::Button::Link)
        .on_press(Message::HidePollutants);

        let header = widget::Row::new()
            .align_y(cosmic::iced::Alignment::Center)
            .push(
                widget::container(widget::text::heading(crate::fl!("air-quality-index")))
                    .width(cosmic::iced::Length::Fill)
                    .align_x(cosmic::iced::alignment::Horizontal::Center),
            )
            .push(close_btn);

        col = col.push(header);
```

**After:**
```rust
        col = col.push(Self::subview_header(
            crate::fl!("air-quality-index"),
            Message::HidePollutants,
        ));
```

- [ ] **Step 4.3: Replace the pollen header**

**Before (lines 1484-1503):**
```rust
        let close_btn = widget::button::custom(
            widget::Row::new()
                .spacing(spacing.space_xxxs)
                .align_y(cosmic::iced::Alignment::Center)
                .push(widget::text::body(crate::fl!("air-quality-close")))
                .push(widget::icon::from_name("go-next-symbolic").size(16)),
        )
        .class(cosmic::theme::Button::Link)
        .on_press(Message::HidePollen);

        let header = widget::Row::new()
            .align_y(cosmic::iced::Alignment::Center)
            .push(
                widget::container(widget::text::heading(crate::fl!("label-pollen")))
                    .width(cosmic::iced::Length::Fill)
                    .align_x(cosmic::iced::alignment::Horizontal::Center),
            )
            .push(close_btn);

        col = col.push(header);
```

**After:**
```rust
        col = col.push(Self::subview_header(
            crate::fl!("label-pollen"),
            Message::HidePollen,
        ));
```

- [ ] **Step 4.4: Replace the locations header**

**Before (lines 1549-1561):**
```rust
        // Back button
        let back_btn = widget::button::custom(
            widget::Row::new()
                .spacing(spacing.space_xxxs)
                .align_y(cosmic::iced::Alignment::Center)
                .push(widget::icon::from_name("go-previous-symbolic").size(16))
                .push(widget::text::body(crate::fl!("locations-back"))),
        )
        .class(cosmic::theme::Button::Link)
        .on_press(Message::HideLocations);

        col = col.push(back_btn);
        col = col.push(widget::divider::horizontal::default());
```

**After:**
```rust
        col = col.push(Self::subview_header(
            crate::fl!("section-saved-locations"),
            Message::HideLocations,
        ));
        col = col.push(widget::divider::horizontal::default());
```

Title source: `section-saved-locations` already exists (the settings section header for the same content) and translates as "Saved locations." Reusing this key avoids inventing a new string and aligns the sub-view title with the settings section label users already see.

- [ ] **Step 4.5: Remove the now-unused `locations-back` key**

Edit `i18n/en/cosmic_ext_applet_tempest.ftl`: delete the `locations-back = Back` line (currently line 39).

Edit `i18n/en-US/cosmic_ext_applet_tempest.ftl`: delete the `locations-back = Back` line (currently line 163).

Do not touch the other locale .ftl files — the Weblate hook blocks edits. The `locations-back` keys in `cs/`, `de/`, `es-ES/`, `fr/`, `hu/`, `pl/`, `pt-BR/`, `ru/`, `sv/`, `uk/`, `zh-Hans/` become orphan keys; Weblate will reconcile.

Run:
```bash
grep -rn "locations-back" src/ i18n/en/ i18n/en-US/
```
Expected: no matches.

- [ ] **Step 4.6: Build and visually verify**

```bash
just check && just install-dev
```

Open the popup. From Current tab:
- Tap the AQI row → Pollutants sub-view opens with centered "Air quality index" title and a close (×) button on the right. Tap close → returns to Current.
- Tap the pollen row → Pollen sub-view opens with centered "Pollen" title and a close (×) button on the right. Tap close → returns to Current.

Save a second location to enable the location picker. Then tap the location header at the top of the popup:
- Saved Locations sub-view opens with centered "Saved locations" title and a close (×) button on the right. Tap close → returns to Current.

Confirm:
- All three sub-views have the same header shape and identical close affordance
- Close icon is the small × glyph, not a right-chevron
- Tab key still cycles focus through the close button

- [ ] **Step 4.7: Commit**

```bash
git add src/applet.rs i18n/en/cosmic_ext_applet_tempest.ftl i18n/en-US/cosmic_ext_applet_tempest.ftl
git commit -m "$(cat <<'EOF'
unify sub-view headers around close-on-right pattern

introduces a shared subview_header helper used by pollutants, pollen,
and saved locations. all three now render a centered title with a
window-close-symbolic icon button on the right, matching the pattern
that landed in 2.8.2 for air quality. drops the locations-back fluent
key in en and en-US; other locales will sync via weblate.
EOF
)"
```

---

## Task 5: Bump to 2.8.3

**Why:** Ship the conformance pass as a patch release.

**Files:**
- Modify: `Cargo.toml:3` (`version = "2.8.2"` → `"2.8.3"`)
- Modify: `Cargo.lock` (regenerated)
- Modify: `res/com.vintagetechie.CosmicExtAppletTempest.metainfo.xml` (new `<release>` entry)
- Modify: `CHANGELOG.md` (new entry + bottom link)
- Modify: `README.md` (changelog excerpt)

- [ ] **Step 5.1: Bump `Cargo.toml`**

Edit `Cargo.toml:3`:
```toml
version = "2.8.2"
```
to:
```toml
version = "2.8.3"
```

- [ ] **Step 5.2: Regenerate `Cargo.lock`**

```bash
cargo update -p cosmic-ext-applet-tempest --precise 2.8.3 2>/dev/null || cargo generate-lockfile
```

If both fail (no network or workspace issue), fall back to:
```bash
cargo check
```
which updates the lockfile in place.

Confirm `Cargo.lock` now has `version = "2.8.3"` for the `cosmic-ext-applet-tempest` package.

- [ ] **Step 5.3: Add `<release>` entry in metainfo**

Open `res/com.vintagetechie.CosmicExtAppletTempest.metainfo.xml`. Find the `<releases>` block and insert a new entry as the first child (newest first per AppData convention). Use today's ISO date.

```xml
<release version="2.8.3" date="2026-05-14">
  <description>
    <p>COSMIC compliance polish.</p>
    <ul>
      <li>Text sizes follow the COSMIC type scale.</li>
      <li>Scrollable surfaces no longer clip against the scrollbar.</li>
      <li>Forecast table columns share a width contract; header reads as a heading.</li>
      <li>Sub-view headers are consistent across pollutants, pollen, and saved locations.</li>
    </ul>
  </description>
</release>
```

Voice check: sentence case, second person not used (release notes are descriptive), no em-dashes, no Oxford comma issues, no emojis.

- [ ] **Step 5.4: Update `CHANGELOG.md`**

Add a new section at the top under any banner, before the previous `## [2.8.2]` entry:

```markdown
## [2.8.3] - 2026-05-14

### Changed
- Text sizes across the popup now use the COSMIC role helpers (`title1`, `title4`, `body`, `caption`) instead of hand-rolled pixel values, so the popup tracks the system typography scale.
- Scrollable surfaces (main popup, alert descriptions) now reserve right padding so the scrollbar no longer clips content.
- Forecast table header and rows share column-width constants; header cells use the `heading` role per the libcosmic table contract.
- Pollutants, pollen, and saved locations sub-views now share one header pattern: centered title with a close button on the right.

### Removed
- Unused `locations-back` fluent key.
```

Add the version link at the bottom of the file, matching the existing pattern. For a GitLab repo:

```markdown
[2.8.3]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/compare/2.8.2...2.8.3
```

- [ ] **Step 5.5: Mirror the changelog excerpt in `README.md`**

Open `README.md`, find the changelog section, prepend the new 2.8.3 block in the same shape as the prior 2.8.2 block.

- [ ] **Step 5.6: Verify and commit**

```bash
just check
grep -n '2\.8\.3' Cargo.toml Cargo.lock res/com.vintagetechie.CosmicExtAppletTempest.metainfo.xml CHANGELOG.md README.md
```
Expected: every file shows the new version exactly once where intended.

```bash
git add Cargo.toml Cargo.lock res/com.vintagetechie.CosmicExtAppletTempest.metainfo.xml CHANGELOG.md README.md
git commit -m "$(cat <<'EOF'
bump to 2.8.3

cosmic compliance polish: typography role helpers, scrollable padding,
shared forecast column widths, unified sub-view headers.
EOF
)"
```

- [ ] **Step 5.7: Final pre-tag verification**

Before tagging the release, run the full mode-toggle pass from `cosmic-style.md`:

```bash
just install-dev
```

Restart the panel. Walk the matrix:
- Light theme, dark theme — confirm typography hierarchy reads correctly in both
- Shape mode: round, slightly round, square — confirm no widget hard-codes a radius
- High contrast on, high contrast off — confirm dividers, buttons, and the new shared sub-view headers all switch cleanly

Popup is fixed at 480 px wide; the 344 px narrow-window context-drawer threshold is not exercised. Note this rather than treating it as a coverage gap.

Spot-check one long localized condition string in the Forecast tab (switch locale to `cs/`, observe a multi-word condition like "Polojasno s občasnými přeháňkami"): confirm the existing `EllipsizeHeightLimit::Lines(1)` constraint still produces a readable row.

If everything passes, the branch is ready to merge to `main` and tag `2.8.3`. Tag/push is out of scope for this plan.

---

## Self-review

Spec coverage check (against `2026-05-14-cosmic-conformance-2-8-3-design.md`):

- Spec commit 1 (typography) → Task 1 covers all 8 listed sites plus the 1969 inline `settings-min` (body) and the 2044 version footer (caption). 1891 added since the spec only listed it implicitly. ✓
- Spec commit 2 (scrollable padding) → Task 2, both sites. Drops bottom `space_m` from outer column per spec. ✓
- Spec commit 3 (forecast table contract) → Task 3. Shared `COL_*` constants, icon wrapping, header `align_y` + padding, caption → heading. ✓
- Spec commit 4 (locations sub-view) → Task 4. Helper introduced, applied to all three sub-views, `go-next-symbolic` → `window-close-symbolic` corrected, `locations-back` key removed instead of renamed (better fit for the Weblate-avoidance posture, uses existing `section-saved-locations` for the title). Deviation from spec: spec proposed rename to `locations-close`, plan removes the key and reuses an existing one. This is strictly less Weblate churn and was offered as an alternative in the spec. ✓
- Spec commit 5 (version bump) → Task 5. All five files. ✓

Placeholder scan: no TBDs, no "implement later," no orphan type references. Helper signature `Self::subview_header(title: String, on_close: Message)` matches every callsite.

Type consistency: `subview_header` returns `Element<'static, Message>`; callsites push it onto a `Column<'_, Message>` which accepts `'static` lifetimes. `Message::HidePollutants`/`HidePollen`/`HideLocations` are existing variants of the existing `Message` enum.

Ambiguity check: Task 1's disambiguation rule for new `.size(11)/.size(13)` sites is explicit (subdued → caption; inline-with-input → body). Task 4 picks `section-saved-locations` as the locations sub-view title, not a new key.

One known deviation from spec, documented above: `locations-back` is removed (and `section-saved-locations` reused) rather than renamed to `locations-close`. The spec offered this as an alternative; the plan picks the lower-Weblate-impact path.
