# Tempest

A weather applet for COSMIC Desktop. Panel display, tabbed popup with current conditions, hourly and 7-day forecasts, weather alerts, air quality. No account, no API key.

![Current conditions](screenshots/tempest-main.png)

<details>
<summary><strong>More screenshots</strong></summary>
<br>

| 7-Day Forecast | Weather Alerts |
|----------------|----------------|
| ![7-Day](screenshots/tempest-7day.png) | ![Alerts](screenshots/tempest-alerts.png) |

| Saved Locations |
|-----------------|
| ![Locations](screenshots/tempest-locations.png) |

</details>

## Data sources

Weather data comes from [Open-Meteo](https://open-meteo.com/). No key required.

In Japan, current temperature is pulled from JMA's AMeDAS ground station network. Open-Meteo's blended model runs several degrees cold in East Asia. AMeDAS is the authoritative source there.

Air quality defaults to Open-Meteo's satellite-derived pipeline. Satellite AQI reads meaningfully different from ground stations outside Europe. Seoul's satellite AQI runs roughly double what aqicn.org shows. Paste a free [aqicn.org](https://aqicn.org/data-platform/token/) token into Settings if you want ground-station readings. Europe stays on Open-Meteo so the EU scale is preserved.

Weather alerts come from NWS (US), ECCC (Canada), MeteoAlarm (EU), and BOM (Australia). Desktop notifications optional.

Location resolves by IP geolocation, city search, or manual coordinates. Bookmark up to eight locations and switch from the popup header.

## Architecture

Weather logic, API calls, region detection, and network monitoring live in a standalone library crate, `tempest-core`. The applet is the frontend. The split was done in 2.6.0 and moved roughly 1,700 lines out of the applet tree.

Network failures retry with exponential backoff (5s, 15s, 30s, 60s). The applet listens to NetworkManager over D-Bus and refreshes immediately when connectivity comes back. HTTP requests time out at 15 seconds so dead connections don't hang the UI.

Panel elements toggle independently: temperature, weather icon, AQI, pressure, dew point, sunrise, sunset. The popup is tabbed: current, hourly, 7-day, alerts, settings. Settings persist. The applet respects the system's 12/24 hour time preference.

## Install

COSMIC Store: search for Tempest under Applets.

From source:

```bash
git clone https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest
cd cosmic-ext-applet-tempest
just build-release
sudo just install
```

`.deb` and `.rpm` builds:

```bash
just build-deb && sudo just install-deb    # Debian/Ubuntu
just build-rpm && sudo just install-rpm    # Fedora/openSUSE
```

Vendored builds: `just vendor && just vendor-build`

## Configuration

Applet, Settings tab. Location mode, units, refresh interval, alert toggles, panel display options. Auto-detects location on first run, falls back to New York if detection fails.

## Development

```bash
just build-debug    # debug build
just check          # clippy
just check-json     # LSP-compatible output
```

## Translations

[![Translation status](https://hosted.weblate.org/widget/tempest/tempest/multi-auto.svg)](https://hosted.weblate.org/engage/tempest/)

Czech, French, German, Hungarian, Polish, Portuguese (Brazil), Russian, Simplified Chinese, Swedish, Ukrainian. More in progress on [Weblate](https://hosted.weblate.org/engage/tempest/).

Translators: lorduskordus (Czech), therealmate (Hungarian), VandaL (Polish), Marco Agostini (Portuguese/Brazil), FaNToMaSikkk (Russian), Geeson Wan (Simplified Chinese), bittin (Swedish), Димко (Ukrainian).

## Changelog

### 2.8.2

Polish pass to bring the popup in line with the COSMIC design spec in Figma. The Air Quality view now leads with a centered title and a Close button on the right, replacing the old Back button on the left. Spacing and icon sizes across the popup, the alerts panel, and the loading and error states are tuned to match the reference. The popup is a touch narrower, and alert descriptions resize to fit instead of always reserving a fixed block of space. No new features and no changes to the data Tempest pulls or how it pulls it.

### 2.8.1

Popup adopts libcosmic's typography presets, list primitives, and spacing tokens. Text picks up theme-driven font weight and line-height instead of literal pixel sizes. Saved locations, active alerts, and pollutants now render through the standard list widget. Settings unit selectors use the standard settings row layout. README rewritten for a technical audience.

### 2.8.0

Open-Meteo's blended model runs several degrees cold in East Asia and its satellite-derived AQI reads nowhere near what ground stations report. Reporters in Tokyo and Seoul caught the temperature off by 3-5°C and Seoul's AQI roughly double what aqicn.org was showing. Current temperature in Japan now comes from JMA's AMeDAS network. Air quality picks up an optional aqicn.org token that sources from ground monitoring stations globally outside Europe. Europe stays on Open-Meteo so the EU scale is preserved.

### 2.6.0

Moved all the weather logic, API calls, region detection, and network monitoring into a standalone library crate, `tempest-core`. The applet is the frontend now, roughly 1,700 lines lighter. Nothing changes for users. The codebase is easier to work with and the core logic is reusable.

### 2.4.3

The applet would give up if the network wasn't ready at boot. VPN still connecting? Enjoy staring at "ERR" for 15 minutes. Now it retries failed fetches with exponential backoff (5s, 15s, 30s, 60s) and listens to NetworkManager over D-Bus for instant refresh when connectivity comes back. HTTP requests also have a 15-second timeout so dead connections don't hang forever. Falls back gracefully if NM isn't available.

Older releases: [CHANGELOG.md](./CHANGELOG.md).

## License

GPL-3.0-only. See [LICENSE](./LICENSE).

## Author

John Crenshaw, [blog.vintagetechie.com](https://blog.vintagetechie.com)
