# Changelog

All notable changes to Tempest will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [2.9.3] - 2026-06-11

### Changed

- A transient network failure now keeps the last good weather on screen instead
  of replacing the popup with the full error view. The "Updated at" line gains a
  small "couldn't refresh" note and a warning glyph; the panel keeps showing the
  last reading, and air quality holds its last value through the blip. The full
  error view (icon, "Failed to load", Retry) now appears only when there is no
  cached weather at all, and the stale note clears on the next successful fetch.
- Air quality attribution now names the source actually used (Open-Meteo or
  aqicn.org) instead of inferring it from settings.
- weathervane updated to 0.8 for the precipitation-unit and air-quality-source
  accessors the above reads from.
- Internal deduplication and maintainability sweep: a single shared helper each
  for manual-location assignment, location detection, the pollen species table,
  precipitation-unit derivation, saved-location coordinate checks, the meteogram
  polyline and max folds, and tab selection. Plan-artifact comment tags are
  replaced with plain-English rationale. No user-facing change.

### Fixed

- The saved-locations limit now shows an inline "Saved locations are full" note
  instead of silently clearing your search when you try to add a ninth.
- The "Updated at" time is restored from the last successful fetch at startup,
  rather than reading blank until the first refresh completes.
- The 24-hour graph clamps to 24 columns so a longer hourly payload can't
  overdraw the chart.

## [2.9.2] - 2026-06-11

### Changed

- The 24-hour graph no longer redraws as you move the mouse over the popup; it
  updates only when the weather, the hour, the time format, or the theme changes.
- The panel icon appears immediately at startup instead of briefly waiting on a
  display-size check.
- Typing in the refresh-interval and air-quality token settings no longer saves
  on every keystroke — changes are saved when you press Enter or close the popup.

## [2.9.1] - 2026-06-11

### Fixed

- Notifications with accented or non-ASCII alert text no longer crash the applet.
- The pressure-unit setting is remembered correctly after a restart.
- Switching location or refreshing quickly no longer briefly shows the previous
  location's weather before the new data lands.
- Opening Settings or Alerts from inside the pollutants, pollen, or
  saved-locations views now returns to the tab you chose.

## [2.9.0] - 2026-06-09

### Added

- Meteogram: a YR.no-style 24-hour graph in a new Graph tab. The top panel
  carries the temperature line over auto-scaled precipitation bars; the
  bottom panel carries the sustained- and gust-wind lines. Weather symbols,
  a labeled time axis, a now-marker, and per-hour night shading run across
  both panels. It is the codebase's first iced canvas, sized to fit the
  480px popup (which now scrolls). Toggle in Settings, on by default.
- The Hourly tab now shows per-hour wind speed and precipitation amount
  next to the columns it already had.
- Pride Month accent: the Philadelphia eight-stripe flag as a thin bar
  across the top of the popup and under the panel readout, every June. On
  by default, with a toggle in Settings.

### Changed

- weathervane updated to 0.5 for the per-hour wind and precipitation data
  the meteogram and the enriched Hourly grid read from.
- Translation updates: Czech, Swedish, and Simplified Chinese.

## [2.8.6] - 2026-06-07

### Changed

- Tempest is now self-distributed. It is no longer on the COSMIC Store;
  install from the AUR (`cosmic-ext-applet-tempest`) or the self-hosted
  Flatpak remote at https://vintagetechie.gitlab.io/flatpak.
- Flatpak users on the old cosmic-utils remote must switch origins; the new
  remote is a different origin and will not auto-update.

No code changes in this release; the applet is identical to 2.8.5.

## [2.8.5] - 2026-05-19

### Changed

- Location section in Settings moves to the canonical
  `settings::section` pattern that the other six sections already
  use. The 2.8.4 pass left it hand-rolled because its
  auto-vs-manual conditional didn't fit the single-chain section
  builder. This release just adds rows conditionally to the
  section instead, so Location reads as a titled card like the
  rest of the tab.
- Save-location and remove-saved-location icon buttons in the
  search-results list and Saved locations card pick up tooltips
  (`tooltip-save-location`, `tooltip-remove-saved-location`). Both
  buttons were icon-only with no label or hint until now.
- `en-US` locale gets the popup header tooltip keys
  (`tooltip-refresh`, `tooltip-alerts`, `tooltip-settings`) that
  2.8.4 added only to `en`. Same for the new save and remove
  keys. Saved locations and Air quality section headers in
  `en-US` drop ALL CAPS for sentence case, matching `en`.
- Internal refactor of `render_settings_tab` from a 248-line
  block to seven per-section helpers
  (`render_location_section`, `render_units_section`,
  `render_updates_section`, `render_aq_section`,
  `render_panel_display_section`, `render_support_section`,
  `render_saved_locations_section`). Adding a setting now touches
  one small method instead of scrolling through the whole tab.
- Internal refactor of `update` from a 465-line message
  dispatch to a thin match that delegates the gnarly arms
  (TogglePopup, RefreshWeather, WeatherUpdated, ToggleAutoLocation,
  SystemTimeConfig, SelectLocation, SaveLocation, LocationDetected)
  to private `handle_*` methods. The 11 inline copies of
  `Task::perform(async { Message::RefreshWeather }, Action::App)`
  collapse to `Self::refresh_task()`.
- Panel button orientation branch no longer duplicates child
  pushes between Row and Column. Children collect into a `Vec`
  once and dispatch to `Row::with_children` or
  `Column::with_children` based on `is_horizontal()`.

### Fixed

- Stale `tempest-core` references in `src/weather.rs` and
  `src/network.rs` doc comments rename to `weathervane`. The
  crate was renamed months ago and these two comments were the
  last places the old name lingered.

## [2.8.4] - 2026-05-15

### Changed

- Settings tab migrates six of seven sections (Saved Locations,
  Units, Updates, Air Quality, Panel Display, Support) to libcosmic's
  canonical `settings::section` pattern. Each section renders as a
  titled card. Location stays hand-rolled because its conditional
  auto-vs-manual branch doesn't fit the single-chain section
  builder. Section header strings drop ALL CAPS for sentence case
  now that they render at `text::title4` weight.
- AQI and pollen rows in the Current tab move from
  `widget::button::custom(...).class(Button::Text)` to
  `widget::list::button` wrapped in a single-row
  `widget::list_column`, picking up the rounded corners and hover
  treatment cosmic-files uses for tappable list rows.
- Sub-view headers (Pollutants, Pollen, Saved Locations) swap the
  X-close on the right for a back arrow (`go-previous-symbolic`)
  on the left, matching the drill-in navigation convention used
  in cosmic-settings, cosmic-store, cosmic-files, and cosmic-edit.
  Sub-view titles bump to `text::title4`.
- Panel button icons and temperature label track the panel size
  tier. Icons pull from `self.core.applet.suggested_size(true)`.
  Text picks a role per tier (heading for XS, title3 for M,
  title2 for L, title1 for XL). The pipe separator between chips
  drops out, since row spacing already separates them.
- Popup header buttons (refresh, alerts, settings) wrap in
  `widget::tooltip::tooltip` with new Fluent keys. Alerts button
  drops the misused `Button::Destructive` class. The icon swap
  to `dialog-warning-symbolic` already flags active alerts, and
  destructive is reserved for delete and shutdown.
- Temperature, Measurement, and Pressure unit selectors
  restructure from `settings::item(label, segmented_control)` to
  a Column with label above and segmented control at full width
  below. The section card padding was leaving Pressure's three-
  button control too narrow to fit three-character labels.
- Hero icons in loading, error, and no-active-alerts states bump
  from 40 to 48 px. Hourly and forecast condition icons bump
  from 20 to 24 px. Both land on Freedesktop symbolic scale
  points.
- Pollutant unit symbol centralizes on a `UG_PER_M3` const with
  proper Unicode `µg/m³`. Panel error label routes through a new
  `panel-error` Fluent key. Accent color recedes from routine
  subtitles, hints, and the version label, kept only on the
  aqicn and pollen attributions where it communicates a content
  requirement.
- Other title-case stragglers (Refresh Interval, Unhealthy for
  Sensitive Groups, Very Unhealthy, Cloud Cover, Auto-select
  Units) drop to sentence case to match the writing voice.

### Fixed

- Closing the popup while a sub-view was open could leave the
  sub-view active on next popup open. `Message::PopupClosed` now
  resets all three `showing_*` flags.

## [2.8.3] - 2026-05-14

### Added

- Pollen forecast for European locations via CAMS (Copernicus
  Atmosphere Monitoring Service), powered by weathervane 0.4. The
  Current tab shows the highest-severity active species with a
  caption counting the rest. A new Pollen sub-view lists every
  species with its grain count and dims off-season species.

### Changed

- Popup text sizes pick up the cosmic typography role helpers
  (`text::title1`, `text::title4`, `text::body`, `text::caption`)
  instead of hand-rolled pixel values, so the scale tracks system
  theme changes.
- Main popup and alert description scrollables wrap their content in
  a padded container before passing it to `widget::scrollable`,
  matching the libcosmic context_drawer convention. The scrollbar
  no longer overlays content on the right edge.
- Forecast tab header and rows share a single set of column-width
  constants, with matched alignment, padding, and spacing. Header
  cells switch from the caption role to the heading role per the
  libcosmic table contract.
- Saved locations sub-view picks up the centered-title-with-close
  header that pollutants and pollen already use, so the three
  sub-views render consistently. The close button uses
  `window-close-symbolic` everywhere.

### Removed

- Unused `locations-back` and `air-quality-close` fluent keys (the
  new icon-only close button replaces the labelled text buttons).

## [2.8.2] - 2026-05-13

### Changed

- Air Quality view header redesigned. The top-left "Back" button is
  replaced with a centered "Air Quality Index" heading and a "Close"
  affordance on the right, matching the read-then-dismiss direction the
  rest of the panel uses.
- Popup density tightened. Header and section padding moves from
  space_xxs to space_xs/space_m where it read cramped. Loading and
  error illustration icons drop from 48 to 40. Alert event icons drop
  from 20 to 16. The header trend icon drops from 18 to 16. Popup
  width drops from 520 to 480.
- Alert description body, aqicn attribution, and expiry timestamps
  adopt the cosmic-theme caption typography preset instead of literal
  size values.
- Alert descriptions wrap to a flexible scroll region capped at 160
  max-height instead of a fixed 100. Short alerts no longer leave
  dead space and long alerts get more room before scrolling kicks in.

## [2.8.1] - 2026-04-27

### Changed

- Popup adopts libcosmic's typography presets and widget::list_column
  primitives. Text picks up theme-driven font weight, line-height,
  and accessibility scaling instead of being frozen at literal pixel
  sizes. Saved locations, active alerts, and the Air Quality
  pollutant list all render through the standard list primitive
  instead of hand-rolled rows-plus-divider loops.
- Settings unit selectors collapse to settings::item, matching the
  rest of the settings layout.
- Every remaining literal spacing and padding in the popup moves to
  the matching cosmic-theme spacing token. The fixed 420 popup width
  and 60 refresh-input width come out so layout handles sizing.
- README rewritten for a technical audience.

## [2.8.0] - 2026-04-21

### Note

Maintenance mode framing from 2.7.0 is lifted. Two reporters on issue
#125 showed Open-Meteo running several degrees cold in Tokyo and Seoul
and its satellite-derived AQI running 50-80 points high against ground
stations. Worth a release, so this isn't maintenance-only anymore.

### Added

- JMA AMeDAS temperature override for Japan. When coordinates fall
  inside Japan the current temperature is pulled from the nearest
  AMeDAS station instead of Open-Meteo's blended model. Forecast and
  hourly stay on Open-Meteo. No setting required, transparent to the
  user beyond the improved accuracy.
- Optional aqicn.org token for ground-station air quality worldwide
  outside Europe. A free token from aqicn.org/data-platform/token/
  goes in Settings, under the new Air Quality section. Europe stays
  on Open-Meteo so the European AQI scale is preserved. Attribution
  line appears under the AQI when aqicn is the source.

### Changed

- Bumped to weathervane 0.3.0 which carries the JMA override and the
  aqicn hybrid plumbing this release depends on. `fetch_air_quality`
  gained an optional token parameter, which is why the library is a
  major-minor bump rather than a patch.

## [2.7.0] - 2026-04-09

### Note

Tempest is moving into maintenance mode after this release. I'll keep
up with bug fixes and library API churn but I'm not planning new
features. The applet does what I built it to do.

### Added

- Recovery from suspend on systems where NetworkManager doesn't fire
  reliably on resume. The applet now subscribes to systemd-logind's
  PrepareForSleep signal, resets the HTTP client connection pool on
  the resume edge, and refreshes weather without requiring a manual
  Retry. Closes #124.

### Changed

- Caught up to libcosmic API changes (Subscription import path moved
  off cosmic::iced_futures, iced_core moved under cosmic::iced::core,
  widget::row and widget::column now require children up front).
- Bumped to weathervane 0.2.0 which carries the underlying error
  variant breakout and HTTP client hardening this release relies on.

## [2.6.0] - 2026-03-22

### Changed
- Extracted all weather logic, API calls, region detection, and network monitoring into the tempest-core library crate
- The applet is now a thin frontend over the shared library (~1,700 lines removed)
- Unit types, weather data models, and location types are now imported from tempest-core
- Weather condition descriptions and AQI categories now match on typed enums instead of raw integers
- Dropped direct dependencies on reqwest, quick-xml, zbus, urlencoding, serde_json, and anyhow

### Added
- Translation updates from Weblate for Czech, Hungarian, Polish, Swedish, Ukrainian, and Simplified Chinese

## [2.5.0] - 2026-03-07

### Added
- Saved locations: bookmark up to 8 locations and switch between them from the popup header
- Location switcher drill-down view accessible by tapping the location name
- Bookmark button on search results in settings
- Current location auto-seeded into saved locations on first run

### Changed
- Streamlined panel rendering by precomputing optional display strings
- Converted manual indexing loops in weather parsing to iterator chains
- Removed 17 dead i18n strings
- Cleaned up redundant comments and simplified D-Bus signal filtering

## [2.4.3] - 2026-03-05

### Fixed
- Applet now retries with exponential backoff when weather fetch fails at startup
- Added NetworkManager D-Bus listener to trigger immediate refresh on connectivity changes
- HTTP requests now have a 15-second timeout to prevent hanging on dead connections

## [2.4.2] - 2026-03-05

### Changed
- Migrated project hosting from Codeberg to GitLab
- Added GitLab CI/CD pipeline for automated .deb builds on release tags
- Added lightweight merge request pipeline for merge gating

## [2.4.1] - 2026-02-27

### Added
- Weather codes for freezing drizzle and freezing rain conditions

### Changed
- Updated translations for Czech, Chinese (Simplified), German, Hungarian, Polish, Portuguese (Brazil), Swedish, Ukrainian, and English (US)

## [2.4.0] - 2026-02-23

### Added
- Pressure unit selector (hPa, inHg, PSI) in settings with auto-units support
- Ukrainian metainfo and desktop translations

### Fixed
- Long condition text overflowing the 7-day forecast widget border (uses libcosmic ellipsis)
- Hardcoded UI strings now routed through Fluent for proper localization

## [2.3.3] - 2026-02-20

### Added
- Portuguese (Brazil) translation
- Czech translation for appstream metainfo (summary, description, screenshot captions)
- weather-fetch-error string for Czech, Hungarian, Polish, Swedish, and Chinese

### Changed
- Refined Czech tab labels and Hungarian panel display wording

## [2.3.2] - 2026-02-09

### Fixed
- Error messages in the popup now show a user-friendly string instead of raw API URLs that could leak coordinates in screenshots
- Alert notifications from external APIs (NWS, MeteoAlarm, ECCC, BOM) now have HTML tags stripped and length capped

### Changed
- Widened popup from 440px to 520px
- Rebalanced forecast table column proportions so dates don't wrap

## [2.3.1] - 2026-01-30

### Added
- Polish translation

### Changed
- Improved max popup height calculation
- Updated dependencies

## [2.3.0] - 2026-01-23

### Added
- Hungarian translation
- Russian translation

### Changed
- Improved popup max height calculation for better screen fit
- Increased panel item spacing for readability
- Removed deprecated translation keys

## [2.2.1] - 2026-01-19

### Fixed
- Restored version display and Ko-fi tip button to settings tab

## [2.2.0] - 2026-01-19

### Changed
- Popup height now adapts to screen resolution using cosmic-randr
- Popup shrinks to fit content instead of always filling to max height
- Added bottom padding for better visual balance

## [2.1.0] - 2026-01-16

### Changed
- Switched to tab bar navigation for a cleaner look
- Air quality info now lives in the Current tab with a dedicated pollutants subview
- Times throughout the app now respect the system 12/24 hour preference
- Polished spacing and alignment across all tabs

### Added
- Czech translation

## [2.0.0] - 2026-01-10

### Changed
- Redesigned settings interface with section headers and cleaner layout
- Tab bar now uses COSMIC segmented control with recessed styling
- Temperature and measurement units use segmented controls instead of toggles
- Auto-select units now immediately applies when toggled
- Pinned libcosmic to stable commit for build reliability

## [1.7.3] - 2025-12-20

### Fixed
- Icons not showing up correctly On Arch

### Added
- Support for German(DE)

## [1.7.0] - 2025-12-20

### Added
- Internationalization (i18n) support using Fluent
- All UI strings extracted to translation files
- Foundation for community translations

## [1.6.1] - 2025-12-12

### Added
- Toggle to show/hide AQI in panel display (Settings tab)

### Changed
- Internal code refactoring for improved maintainability
- Shared HTTP client for better connection pooling
- Extracted helper functions to reduce code duplication

## [1.4.0] - 2025-12-05

### Added
- Auto-select temperature and measurement units based on detected location
- Weather alerts for US locations via NWS API

### Changed
- Sync README with current features

### Fixed
- Packaging configuration in justfile

## [1.3.0] - 2025-11-27

### Added
- Tabbed popup interface replacing collapsible sections
- Night icon detection using actual sunrise/sunset times

### Fixed
- Resolved clippy warnings

## [1.2.0] - 2025-11-26

### Changed
- Remember manual location settings between sessions
- Sync measurement units with temperature unit selection

## [1.1.0] - 2025-11-25

### Added
- Air quality data from Open-Meteo API
- AQI displayed in panel alongside temperature
- Collapsible air quality section with PM2.5, PM10, ozone, NO2, CO
- Auto-detects European locations and uses EU AQI scale

### Changed
- UI polish and cleanup
- Improved date formatting in 7-day forecast
- Added weather icons to forecast rows
- Replaced Unicode arrows with proper icons in collapsible headers

## [1.0.2] - 2025-11-24

### Added
- Automated .deb package releases via GitHub Actions
- install-dev target in justfile

### Fixed
- Remove %F argument causing Flatpak launch failure

## [1.0.1] - 2025-11-24

### Fixed
- Changed metainfo component type to desktop-application for COSMIC Store visibility
- Added com.system76.CosmicApplet provides declaration

## [1.0.0] - 2025-11-21

### Added
- Initial production release
- Real-time weather data from Open-Meteo API (no API key required)
- Automatic location detection via IP geolocation
- Current temperature displayed in COSMIC panel
- Detailed popup window with comprehensive weather information:
  - Location name with manual refresh button in header
  - Last updated timestamp with loading spinner
  - Current conditions (temperature, feels-like, humidity)
  - Wind information (speed, direction compass, gusts)
  - UV index and cloud cover percentage
  - Visibility and atmospheric pressure
  - Sunrise and sunset times with timezone support
  - Collapsible hourly forecast (next 12 hours)
  - Collapsible 7-day forecast with high/low temperatures
- Configuration settings:
  - Temperature unit toggle (Fahrenheit/Celsius)
  - Custom location support (latitude/longitude)
  - Adjustable refresh interval
  - Version display
  - Ko-fi support link for donations
- Persistent configuration storage
- Global weather coverage

[2.8.5]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/compare/2.8.4...2.8.5
[2.8.4]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/compare/2.8.3...2.8.4
[2.8.3]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/compare/2.8.2...2.8.3
[2.8.2]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v2.8.2
[2.8.1]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v2.8.1
[2.6.0]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v2.6.0
[2.5.0]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v2.5.0
[2.4.3]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v2.4.3
[2.4.2]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v2.4.2
[2.4.1]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v2.4.1
[2.4.0]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v2.4.0
[2.3.3]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v2.3.3
[2.3.2]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v2.3.2
[2.3.1]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v2.3.1
[2.3.0]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v2.3.0
[2.2.1]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v2.2.1
[2.2.0]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v2.2.0
[2.1.0]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v2.1.0
[2.0.0]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v2.0.0
[1.7.3]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v1.7.3
[1.7.0]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v1.7.0
[1.6.1]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v1.6.1
[1.4.0]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v1.4.0
[1.3.0]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v1.3.0
[1.2.0]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v1.2.0
[1.1.0]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v1.1.0
[1.0.2]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v1.0.2
[1.0.1]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v1.0.1
[1.0.0]: https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest/-/releases/v1.0.0
