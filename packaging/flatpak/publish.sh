#!/usr/bin/env bash
# Build, GPG-sign, and stage the Tempest Flatpak OSTree repo for publishing.
#
# Produces ./public/ (the GitLab Pages artifact):
#   public/repo/                         -- the signed OSTree repo (the Flatpak remote)
#   public/vintagetechie-tempest.flatpakrepo  -- one-click remote descriptor
#
# Usage (run from repo root):
#   FLATPAK_GPG_KEY=<fingerprint> ./packaging/flatpak/publish.sh
#
# Env:
#   FLATPAK_GPG_KEY   GPG key fingerprint/id to sign with (required for a signed remote).
#   GNUPGHOME         Optional; point at a keyring dir (CI imports the key here).
#   STATE_DIR         flatpak-builder state/cache dir (default: .flatpak-builder).
#   BUILD_DIR         flatpak-builder build dir       (default: .flatpak-build).
set -euo pipefail

MANIFEST="com.vintagetechie.CosmicExtAppletTempest.json"
APPID="com.vintagetechie.CosmicExtAppletTempest"
REPO_DIR="public/repo"
STATE_DIR="${STATE_DIR:-.flatpak-builder}"
BUILD_DIR="${BUILD_DIR:-.flatpak-build}"

[ -f "$MANIFEST" ] || { echo "Run from the repo root (missing $MANIFEST)" >&2; exit 1; }
[ -f cargo-sources.json ] || { echo "Missing cargo-sources.json (regenerate with flatpak-cargo-generator.py Cargo.lock -o cargo-sources.json)" >&2; exit 1; }

SIGN_ARGS=()
if [ -n "${FLATPAK_GPG_KEY:-}" ]; then
    SIGN_ARGS=(--gpg-sign="$FLATPAK_GPG_KEY")
else
    echo "WARNING: FLATPAK_GPG_KEY not set -- building an UNSIGNED repo." >&2
fi

mkdir -p public
rm -rf "$BUILD_DIR"

# CI containers usually lack FUSE; pass FLATPAK_BUILDER_EXTRA="--disable-rofiles-fuse" there.
read -ra _FB_EXTRA <<< "${FLATPAK_BUILDER_EXTRA:-}"

flatpak-builder --user --force-clean \
    --state-dir="$STATE_DIR" \
    --repo="$REPO_DIR" \
    "${_FB_EXTRA[@]}" \
    "${SIGN_ARGS[@]}" \
    "$BUILD_DIR" "$MANIFEST"

# Static deltas make first-install downloads smaller/faster; refresh the summary.
flatpak build-update-repo \
    --generate-static-deltas \
    --prune --prune-depth=20 \
    "${SIGN_ARGS[@]}" \
    "$REPO_DIR"

cp packaging/flatpak/vintagetechie-tempest.flatpakrepo public/

echo
echo "Staged Pages artifact in ./public/"
echo "  remote ref:  $(ostree --repo="$REPO_DIR" refs | grep "app/$APPID" || true)"
echo "  descriptor:  public/vintagetechie-tempest.flatpakrepo"
