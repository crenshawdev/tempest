# Changelog

All notable changes to Tempest will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
