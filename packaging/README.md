# Packaging & distribution (maintainer notes)

Tempest is **self-distributed**. After being delisted from the cosmic-utils Flatpak
pointer repo, releases ship through channels owned entirely here:

- **AUR** — `packaging/aur/` (source package, builds from the GitLab release tag)
- **Self-hosted Flatpak remote** — `packaging/flatpak/`, published to GitLab Pages
- **deb / rpm** — existing `cargo deb` / `cargo generate-rpm` (see `res/packaging.just`)

---

## AUR

`packaging/aur/` holds the canonical `PKGBUILD` and `.SRCINFO`. The AUR repo is a
*separate* git repo; this directory is the source of truth that gets copied into it.

**On each release (after the `vX.Y.Z` tag is pushed to GitLab):**

```bash
cd packaging/aur
# 1. bump pkgver, reset pkgrel=1
# 2. refresh the source checksum:
updpkgsums                       # (pacman-contrib) or compute sha256 by hand
# 3. test a clean build:
makepkg -f
namcap PKGBUILD *.pkg.tar.zst    # optional lint
# 4. regenerate metadata:
makepkg --printsrcinfo > .SRCINFO
```

**Publish to the AUR** (needs your AUR account + SSH key registered):

```bash
git clone ssh://aur@aur.archlinux.org/cosmic-ext-applet-tempest.git aur-pkg
cp PKGBUILD .SRCINFO aur-pkg/
cd aur-pkg && git commit -am "Update to X.Y.Z" && git push
```

`depends` (`wayland`, `libxkbcommon`, `openssl`) were derived from the linked + dlopen'd
libraries of the built binary — re-check with `namcap`/`ldd` if dependencies change.

---

## Flatpak remote (GitLab Pages)

App id: `com.vintagetechie.CosmicExtAppletTempest` · published branch: `stable`.

Files:
- `com.vintagetechie.CosmicExtAppletTempest.json` — manifest (repo root)
- `cargo-sources.json` — offline crate/git sources for the sandboxed build (repo root)
- `packaging/flatpak/publish.sh` — build + sign + stage `./public/`
- `packaging/flatpak/vintagetechie-tempest.flatpakrepo` — one-click remote descriptor
- `packaging/flatpak/tempest-repo-signing-key.asc` — public half of the signing key

### When the version changes

1. Bump the manifest `tag` to the new `vX.Y.Z`.
2. Regenerate offline sources (only needed if `Cargo.lock` changed):
   ```bash
   python3 flatpak-cargo-generator.py Cargo.lock -o cargo-sources.json
   ```
   (`flatpak-cargo-generator.py` lives in flathub/flatpak-builder-tools; needs
   `aiohttp`, `PyYAML`, `tomlkit`.)

### Signing key

The remote is GPG-signed. Key fingerprint:

```
D17CA5791848B6FB1816BD3C164E693982646201   "Tempest Flatpak Repo (VintageTechie)"
```

It lives in the local GnuPG keyring (sign-only, no passphrase so CI can sign unattended).
The public half is embedded in the `.flatpakrepo` and exported to
`tempest-repo-signing-key.asc`. **Back up the private key somewhere safe** — losing it
means users must re-add the remote with a new key.

> If you'd rather use your own key, regenerate it, then re-run `publish.sh`, refresh the
> `GPGKey=` line in the `.flatpakrepo`, and re-export the `.asc`.

### Publishing

**Automated (CI, on `vX.Y.Z` tags)** — the `pages` job in `.gitlab-ci.yml` builds, signs,
and deploys. One-time setup in **Settings > CI/CD > Variables**:

| Variable | Value | Flags |
|---|---|---|
| `FLATPAK_GPG_KEY_ID` | `D17CA5791848B6FB1816BD3C164E693982646201` | — |
| `FLATPAK_GPG_PRIVATE_KEY` | output of the command below | Masked, Protected |

```bash
gpg --export-secret-keys D17CA5791848B6FB1816BD3C164E693982646201 | base64 -w0
```

**Manual / local fallback:**

```bash
FLATPAK_GPG_KEY=D17CA5791848B6FB1816BD3C164E693982646201 ./packaging/flatpak/publish.sh
# stages ./public/  (repo + .flatpakrepo) — upload via the GitLab Pages API if not using CI
```

### ⚠️ Confirm the Pages URL

The `.flatpakrepo` `Url=` and the CI's published URL assume:

```
https://vintagetechie.gitlab.io/cosmic-ext-applet-tempest/repo
```

GitLab may assign a **unique/random Pages domain** by default. After the first Pages
deploy, check **Settings > Pages** for the real URL and, if it differs, update `Url=` in
`packaging/flatpak/vintagetechie-tempest.flatpakrepo` (and re-deploy). The remote will not
work until this matches.
