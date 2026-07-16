# cosmic-ext-applet-tempest

Weather applet for COSMIC Desktop with automatic location detection

## Installation

> Tempest is **self-distributed** (delisted from the cosmic-utils Flatpak repo). Install it
> from one of the channels below, or build from source.

### Arch Linux (AUR)

```bash
paru -S cosmic-ext-applet-tempest   # or: yay -S cosmic-ext-applet-tempest
```

### Flatpak (self-hosted remote)

Tempest ships through the jcrenshaw.dev Flatpak remote — add it once and you get this and
every other jcrenshaw.dev app:

```bash
flatpak remote-add --if-not-exists --from jcrenshaw https://pkg.jcrenshaw.dev/flatpak/jcrenshaw.flatpakrepo
flatpak install jcrenshaw com.vintagetechie.CosmicExtAppletTempest
```

Already have Tempest from the old `vintagetechie.gitlab.io/flatpak` remote or the old
cosmic-utils remote? Those are retired origins and will **not** auto-update. Switch over
once:

```bash
flatpak uninstall com.vintagetechie.CosmicExtAppletTempest
flatpak remote-delete vintagetechie 2>/dev/null || true
flatpak remote-add --if-not-exists --from jcrenshaw https://pkg.jcrenshaw.dev/flatpak/jcrenshaw.flatpakrepo
flatpak install jcrenshaw com.vintagetechie.CosmicExtAppletTempest
```

### Build from source

Clone the repository:

```bash
git clone https://github.com/crenshawdev/tempest
cd tempest
```

Build and install the project:

```bash
just build-release
sudo just install
```

For alternative packaging methods, use the one of the following recipes:

- `deb`: run `just build-deb` and `sudo just install-deb`
- `rpm`: run `just build-rpm` and `sudo just install-rpm`

For vendoring, use `just vendor` and `just vendor-build`

## Contributing

A [justfile](./justfile) is included with common recipes used by other COSMIC projects:

- `just build-debug` compiles with debug profile
- `just run` builds and runs the application
- `just check` runs clippy on the project to check for linter warnings
- `just check-json` can be used by IDEs that support LSP

## License

Code is distributed with the [GPL-3.0-only license][./LICENSE]

