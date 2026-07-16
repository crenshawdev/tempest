#!/usr/bin/env bash
# Guard: the flatpak manifest must pin the exact tag being released.
#
# History: v2.9.1-v2.9.3 shipped with the manifest still on v2.9.0, so the
# flatpak remote silently republished the stale build. This check fails the
# release the moment the manifest's pinned tag drifts from the release tag.
#
# Usage (run from anywhere; resolves the manifest relative to the repo root):
#   packaging/flatpak/assert-manifest-tag.sh <release-tag>
# Exit 0 when the manifest's pinned tag equals <release-tag>, non-zero otherwise.
set -euo pipefail

RELEASE_TAG="${1:?usage: assert-manifest-tag.sh <release-tag>}"

# Repo root = two levels up from this script (packaging/flatpak/ -> repo root).
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
MANIFEST="$REPO_ROOT/com.vintagetechie.CosmicExtAppletTempest.json"

[ -f "$MANIFEST" ] || { echo "FATAL: manifest not found at $MANIFEST" >&2; exit 1; }

# Extract the pinned tag without a jq dependency (the flatpak-builder CI
# container is minimal and may lack jq). The manifest has exactly one "tag" key,
# in modules[0].sources[0], so the first match is authoritative.
MANIFEST_TAG="$(grep -oE '"tag"[[:space:]]*:[[:space:]]*"[^"]+"' "$MANIFEST" | head -1 | sed -E 's/.*:[[:space:]]*"([^"]+)".*/\1/')"

if [ "$MANIFEST_TAG" != "$RELEASE_TAG" ]; then
    echo "FATAL: manifest pins '$MANIFEST_TAG' but releasing '$RELEASE_TAG' -- bump the manifest (and regenerate cargo-sources.json) before tagging." >&2
    exit 1
fi

echo "OK: manifest pins '$MANIFEST_TAG', matching release tag '$RELEASE_TAG'."
