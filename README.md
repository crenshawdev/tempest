# Tempest

A weather applet for COSMIC Desktop. Panel display, tabbed popup with current conditions, hourly and 7-day forecasts, a 24-hour meteogram, weather alerts, air quality. No account, no API key.

![Current conditions](screenshots/tempest-main.png)

<details>
<summary><strong>More screenshots</strong></summary>
<br>

| Hourly Forecast | 7-Day Forecast |
|-----------------|----------------|
| ![Hourly](screenshots/tempest-hourly.png) | ![7-Day](screenshots/tempest-7day.png) |

</details>

## Data sources

Weather data comes from [Open-Meteo](https://open-meteo.com/). No key required.

In Japan, current temperature is pulled from JMA's AMeDAS ground station network. Open-Meteo's blended model runs several degrees cold in East Asia. AMeDAS is the authoritative source there.

Air quality defaults to Open-Meteo's satellite-derived pipeline. Satellite AQI reads meaningfully different from ground stations outside Europe. Seoul's satellite AQI runs roughly double what aqicn.org shows. Paste a free [aqicn.org](https://aqicn.org/data-platform/token/) token into Settings if you want ground-station readings. Europe stays on Open-Meteo so the EU scale is preserved.

Weather alerts come from NWS (US), ECCC (Canada), MeteoAlarm (EU), and BOM (Australia). Desktop notifications optional.

Location resolves by IP geolocation, city search, or manual coordinates. Bookmark up to eight locations and switch from the popup header.

## Architecture

Weather logic, API calls, region detection, and network monitoring live in a standalone library crate, `weathervane`. The applet is the frontend. The split was done in 2.6.0 and moved roughly 1,700 lines out of the applet tree.

Network failures retry with exponential backoff (5s, 15s, 30s, 60s). The applet listens to NetworkManager over D-Bus and refreshes immediately when connectivity comes back. HTTP requests time out at 15 seconds so dead connections don't hang the UI.

Panel elements toggle independently: temperature, weather icon, AQI, pressure, dew point, sunrise, sunset. The popup is tabbed: current, hourly, 7-day, graph, alerts, settings. Settings persist. The applet respects the system's 12/24 hour time preference.

## Install

Tempest is self-distributed. It's not in the COSMIC Store.

### Arch (AUR)

```bash
paru -S cosmic-ext-applet-tempest
```

### Flatpak

Add the VintageTechie remote once. Everything I ship lives there.

```bash
flatpak remote-add --if-not-exists vintagetechie https://vintagetechie.gitlab.io/flatpak/vintagetechie.flatpakrepo
flatpak install vintagetechie com.vintagetechie.CosmicExtAppletTempest
```

Installed from the old cosmic-utils remote? Different origin, so it won't auto-update. Switch once:

```bash
flatpak uninstall com.vintagetechie.CosmicExtAppletTempest
flatpak remote-add --if-not-exists vintagetechie https://vintagetechie.gitlab.io/flatpak/vintagetechie.flatpakrepo
flatpak install vintagetechie com.vintagetechie.CosmicExtAppletTempest
```

### From source

```bash
git clone https://gitlab.com/vintagetechie/cosmic-ext-applet-tempest
cd cosmic-ext-applet-tempest
just build-release
sudo just install
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

### 2.9.0

A graph, finally. The new Graph tab draws a YR.no-style meteogram for the next 24 hours: temperature line over precipitation bars up top, sustained and gust wind below, weather symbols and a now-marker across both, night hours shaded. It's the codebase's first iced canvas, sized to fit the 480px popup, which now scrolls. The Hourly tab picks up per-hour wind and precipitation amount next to the columns it already had. Both lean on weathervane 0.5 for the hourly data. Turn the meteogram off in Settings if you'd rather keep the old tab set. And every June, a thin Philadelphia pride flag rides across the top of the popup and under the panel readout — on by default, one toggle in Settings to turn it off. Plus translation updates for Czech, Swedish, and Simplified Chinese.

### 2.8.6

No code changes — distribution only. Tempest is off the COSMIC Store and self-distributed now: the AUR, and a self-hosted Flatpak remote at https://vintagetechie.gitlab.io/flatpak. Add the remote once and everything I ship lives there. If you installed the Flatpak from the old cosmic-utils remote it's a different origin and won't auto-update, so switch over once. The applet itself is identical to 2.8.5.

### 2.8.5

Settings polish. The Location section moves to the same titled-card layout the other six sections already use, instead of floating loose under a heading. Save-location and remove-saved-location icon buttons grow tooltips so it's obvious what they do. Saved locations and Air quality headers in the en-US locale drop ALL CAPS for sentence case, catching up with what en already had. Under the hood the settings tab splits into one method per section and the message dispatch splits into per-handler methods, so adding a setting or wiring a new message touches one small place instead of scrolling through a few hundred lines. Stale `tempest-core` doc-comment references in `weather.rs` and `network.rs` rename to `weathervane`.

### 2.8.4

Another COSMIC conformance pass. Settings tab sections render as proper titled cards now, matching cosmic-settings. The AQI and pollen rows in the Current tab read as tappable cards with chevron, replacing the flat text-button styling. Sub-view headers swap the X-close on the right for a back arrow on the left, since sub-views are drill-in pages and that's the convention every other COSMIC app uses. Panel button icons and the temperature label scale with the panel size tier instead of pinning to a single size. Popup header buttons (refresh, alerts, settings) wear tooltips. The alerts header button drops the destructive red it was using to flag active alerts. Destructive is reserved for delete and shutdown, and the warning icon swap already conveys the state. Section headers move from caption-with-accent to title4, ALL CAPS settings labels drop to sentence case, and overuse of accent color on routine subtitles is pruned back. The Temperature, Measurement, and Pressure unit selectors get full width below their labels instead of being squeezed into the right half of a row. Plus a small latent bug where closing the popup mid-sub-view could leave it stuck on reopen.

### 2.8.3

Pollen forecast for European locations via CAMS, surfaced in the Current tab. The highest-severity active species leads, with a caption counting the rest, and a new Pollen sub-view breaks out every species with its grain count. Plus a COSMIC compliance polish in the popup. Text sizes pick up the cosmic typography role helpers instead of hand-rolled pixel values, so the scale tracks system theme changes. Scrollable surfaces in the popup and alert descriptions now reserve room for the scrollbar so it stops clipping content. The Forecast tab's header and rows share a single column-width contract, and the header reads as a heading rather than a caption. The saved locations sub-view picks up the same centered-title-with-close header that pollutants and pollen already use, so the three sub-views match.

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

## Trademark

Tempest is an independent, third-party project. It is not affiliated with, endorsed by, or sponsored by System76, Inc. "COSMIC" is a trademark of System76, Inc. Tempest is built for the COSMIC™ desktop using the public libcosmic framework.

## Author

John Crenshaw, [blog.vintagetechie.com](https://blog.vintagetechie.com)
