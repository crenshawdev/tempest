# Tempest 2.8.3 — COSMIC conformance pass

Spec date: 2026-05-14
Target branch: `cosmic-conformance-2-8-3`
Target release: 2.8.3

## Goals

Bring `src/applet.rs` into compliance with the libcosmic and cosmic-style rule sets on three axes that fell out of the 2026-05-14 review:

1. Typography uses cosmic role helpers instead of hand-rolled font sizes, so the popup tracks the system typography scale and theme.
2. Scrollable surfaces are padded against the scrollbar viewport overlap, matching libcosmic's `context_drawer` convention.
3. The three peer sub-views (pollutants, pollen, locations) share one header pattern: centered title with a trailing close button, using `window-close-symbolic`.

## Out of scope

- Writing voice / sentence-case sweep on `i18n/en/cosmic_ext_applet_tempest.ftl`. Deferred to avoid Weblate churn this cycle.
- Misc cleanup items from the review's Other observations (`panel-error` fluent key, .ftl section comment placement).
- Forecast condition ellipsization at 480 px popup width. Tracked as a judgment call; no change.

## Commit plan

Five commits on `cosmic-conformance-2-8-3`, in this order:

1. **swap hand-rolled text sizes for cosmic role helpers** — `src/applet.rs` only.
2. **pad scrollable content so scrollbar stops clipping** — `src/applet.rs` only.
3. **unify forecast table header and row column widths** — `src/applet.rs` only.
4. **align locations sub-view to close-on-right pattern** — `src/applet.rs` plus minor `i18n/en/cosmic_ext_applet_tempest.ftl` rename.
5. **bump to 2.8.3** — version sites listed below.

## Detail per commit

### Commit 1 — typography

All sites in `src/applet.rs`:

- `:619, :630` location header — `text(...).size(18)` → `widget::text::title4(...)`.
- `:645` failed-to-load — `text(crate::fl!("failed-to-load")).size(18)` → `widget::text::title4(...)`.
- `:662` loading — `text(crate::fl!("loading")).size(18)` → `widget::text::title4(...)`.
- `:1611` no-active-alerts — `text(crate::fl!("no-active-alerts")).size(16)` → `widget::text::title4(...)`.
- `:1251` headline temperature — `.size(36)` → `widget::text::title1(...)` (35 px per the COSMIC scale).
- `:1328` AQI headline row — `.size(20)` → `widget::text::title4(...)`.
- `:1385` pollen headline row — `.size(20)` → `widget::text::title4(...)`.
- `:1698, :1833, :1891, :1954, :1969, :1996, :2044` — `.size(11)` / `.size(13)`. Case by case:
  - Subdued metadata, secondary labels → `widget::text::caption(...)` (11.6 px).
  - Inline value text that pairs with a label → `widget::text::body(...)` (14 px).

Preserve any `Text::Accent` style on each site; helpers accept the same `.class(...)` chain.

### Commit 2 — scrollable padding

Two sites in `src/applet.rs`.

`:693` main popup. Today:

```rust
widget::scrollable(column)
```

After:

```rust
let padded = container::Container::new(column).padding([
    0,
    spacing.space_l,
    spacing.space_l,
    spacing.space_l,
]);
widget::scrollable(padded)
```

Drop the bottom `space_m` from the outer column at `:560-565` so we do not double-pad below the scrollable content.

`:1646-1650` alert description. Today the scrollable wraps `widget::text::caption(...)` directly and the outer container applies `[xxxs, 0, xxxs, 0]` (zero on the right). After:

```rust
widget::scrollable(
    container::Container::new(widget::text::caption(...))
        .padding([0, spacing.space_s, spacing.space_s, 0]),
)
```

Keep `.max_height(160.0)` — that is a deliberate vertical constraint, not a spacing-token candidate.

### Commit 3 — forecast table

In `src/applet.rs` around `:1729-1789`. Introduce shared column-width constants at function scope or near the table:

```rust
const COL_DAY:  Length = Length::FillPortion(3);
const COL_ICON: Length = Length::Fixed(24.0);
const COL_HIGH: Length = Length::FillPortion(1);
const COL_LOW:  Length = Length::FillPortion(1);
const COL_COND: Length = Length::FillPortion(2);
```

Header changes:

- Each header cell switches `widget::text::caption(...)` → `widget::text::heading(...)` per the libcosmic table contract (heading is 14 px / 700 weight; caption is for subdued metadata).
- Apply `.width(COL_DAY)`, `.width(COL_HIGH)`, `.width(COL_LOW)`, `.width(COL_COND)` to the four text cells.
- Replace the existing `widget::Space::new().width(spacing.space_m)` icon-column placeholder with `container::Container::new(widget::Space::new()).width(COL_ICON)` so the column is named consistently.
- Add `.align_y(Alignment::Center)` and `.padding([0, spacing.space_xxs])` to the header row so it matches the data rows.

Row changes:

- Each row cell takes the same `COL_*` width as its header peer.
- Wrap the weather icon in `container::Container::new(icon).width(COL_ICON).align_x(Alignment::Center)` so its column is explicit.
- Keep `align_y(Alignment::Center)`, `.padding([0, spacing.space_xxs])`, and `.spacing(spacing.space_xxs)` consistent with the header.

Result: header and rows share exactly the same column contract, padding, and spacing. Alignment becomes a property of the constants, not a coincidence.

### Commit 4 — locations sub-view alignment

`src/applet.rs` and one `i18n` string.

In `src/applet.rs:1540-1576`, replace the `go-previous-symbolic` back button + title pair with the pollutants/pollen pattern: centered title, then `widget::space::horizontal()`, then a close button on the right.

Refactor the sub-view header into a small helper since all three views now use the same shape:

```rust
fn subview_header<'a>(
    title: impl Into<Cow<'a, str>>,
    on_close: Message,
    spacing: &Spacing,
) -> Element<'a, Message> {
    widget::row::with_children([
        widget::text::title4(title).into(),
        widget::space::horizontal().into(),
        widget::button::icon(widget::icon::from_name("window-close-symbolic"))
            .on_press(on_close)
            .into(),
    ])
    .align_y(Alignment::Center)
    .padding([0, spacing.space_xs])
    .spacing(spacing.space_xs)
    .into()
}
```

Adjust pollutants (`:1402-1430`) and pollen (`:1475-1503`) to call this helper, replacing their existing `go-next-symbolic` close icon with `window-close-symbolic` and their `Length::Fill` title-container with `widget::space::horizontal()`.

Add locations sub-view (`:1540-1576`) to use the same helper. Remove the leading back button entirely.

i18n: `locations-back = Back` becomes either:

- Renamed to `locations-close = Close`, or
- Removed entirely and replaced with the existing `air-quality-close = Close` reused as a generic close label.

Decision: rename to `locations-close` and keep per-view keys for translator clarity. Translators see distinct keys per context, which matters in languages where the verb form for "close a list" differs from "close a detail view."

The `air-quality-close` key stays as-is; do not consolidate, for the same translator-clarity reason.

### Commit 5 — version bump to 2.8.3

Per project memory (`/home/john/.claude/projects/-code-cosmic-ext-applet-tempest/memory/MEMORY.md`), update version in:

- `Cargo.toml` (source of truth)
- `Cargo.lock` (`cargo generate-lockfile` or `cargo update`)
- `res/com.vintagetechie.CosmicExtAppletTempest.metainfo.xml` (new `<release>` entry with COSMIC-voice release notes)
- `CHANGELOG.md` (new entry + bottom link)
- `README.md` (changelog section mirror)

Release notes content focus: "Typography and layout polish: text sizes now follow the COSMIC type scale; scrollable surfaces no longer clip against the scrollbar; forecast table columns share a width contract; sub-view headers are consistent across pollutants, pollen, and saved locations."

## Verification

Per commit:

- `just check` clean (lint and fmt).
- `just install-dev` build succeeds.
- Manual sanity check: open the popup, visit each tab and each sub-view, confirm no visible regression vs the 2.8.2 baseline.

Before tagging 2.8.3:

- Mode-toggle pass per cosmic-style.md checklist:
  - Light theme and dark theme.
  - Shape: round, slightly round, square.
  - High contrast on and off.
- Popup is fixed at 480 px width, so the 344 px narrow-window context-drawer path is not exercised. Note this in the verification log rather than treating it as a coverage gap.
- Spot-check the forecast table at a long localized condition string ("Partly cloudy with isolated thunderstorms" or similar long en string) to confirm the existing `EllipsizeHeightLimit::Lines(1)` constraint still produces a readable row.

## Non-goals / explicit deferrals

- No writing-voice fixes in `i18n/en/cosmic_ext_applet_tempest.ftl` beyond the single `locations-back` → `locations-close` rename required for commit 4.
- No new fluent key for `"ERR"` panel fallback at `:809` (Other observations).
- No `.ftl` section comment cleanup (Other observations).
- No change to the forecast condition ellipsization behavior.

These can land in a follow-up writing-voice MR alongside Weblate coordination.
