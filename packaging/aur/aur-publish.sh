#!/usr/bin/env bash
# Publish the current release of cosmic-ext-applet-tempest to the AUR.
#
# Runs entirely on your machine: it signs the AUR push with the SSH key already
# in your agent/keyring, so no secret ever goes to CI.
#
# Usage:
#   packaging/aur/aur-publish.sh                 # version from Cargo.toml
#   packaging/aur/aur-publish.sh --version 2.8.6 # explicit version
#   packaging/aur/aur-publish.sh --dry-run       # build files, print, DON'T push
#   packaging/aur/aur-publish.sh --test-build    # also makepkg -s to compile-check
#
# Requires: Arch tooling (makepkg), git, curl, sha256sum, and an AUR account
# whose SSH key is loaded locally.
set -euo pipefail

PKGNAME=cosmic-ext-applet-tempest
REPO_URL="https://github.com/crenshawdev/tempest"
AUR_REMOTE="ssh://aur@aur.archlinux.org/${PKGNAME}.git"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AUR_DIR="$SCRIPT_DIR"
REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel)"

VERSION="" DRY_RUN=0 TEST_BUILD=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)    VERSION="${2:?--version needs an argument}"; shift 2;;
    --dry-run)    DRY_RUN=1; shift;;
    --test-build) TEST_BUILD=1; shift;;
    -h|--help)    sed -n '2,18p' "$0"; exit 0;;
    *) echo "unknown argument: $1" >&2; exit 2;;
  esac
done

# Preconditions ---------------------------------------------------------------
command -v makepkg   >/dev/null || { echo "error: makepkg not found (run on Arch)" >&2; exit 1; }
command -v sha256sum >/dev/null || { echo "error: sha256sum not found" >&2; exit 1; }
[[ $EUID -ne 0 ]] || { echo "error: do not run as root — makepkg refuses" >&2; exit 1; }

if [[ -z "$VERSION" ]]; then
  VERSION="$(grep -m1 '^version' "$REPO_ROOT/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')"
fi
TAG="v$VERSION"
TARBALL_URL="$REPO_URL/archive/refs/tags/$TAG.tar.gz"

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT

# 1. Fetch the released tarball and hash it -----------------------------------
echo ">> fetching $TARBALL_URL"
curl -fsSL "$TARBALL_URL" -o "$tmp/src.tar.gz" \
  || { echo "error: tarball not found — is tag $TAG pushed?" >&2; exit 1; }
SHA="$(sha256sum "$tmp/src.tar.gz" | awk '{print $1}')"
echo ">> sha256 = $SHA"

# 2. Render the updated PKGBUILD ----------------------------------------------
cp "$AUR_DIR/PKGBUILD" "$tmp/PKGBUILD"
sed -i -E "s/^pkgver=.*/pkgver=$VERSION/"            "$tmp/PKGBUILD"
sed -i -E "s/^pkgrel=.*/pkgrel=1/"                   "$tmp/PKGBUILD"
sed -i -E "s/^sha256sums=\(.*/sha256sums=('$SHA')/"  "$tmp/PKGBUILD"

# 3. Regenerate .SRCINFO from the PKGBUILD ------------------------------------
( cd "$tmp" && makepkg --printsrcinfo > "$tmp/.SRCINFO" )

# 4. Optional real compile check ----------------------------------------------
if [[ $TEST_BUILD -eq 1 ]]; then
  echo ">> test build (makepkg -s, ~10 min)…"
  ( cd "$tmp" && makepkg -sf --noconfirm )
fi

echo ">> PKGBUILD diff vs committed:"
diff -u "$AUR_DIR/PKGBUILD" "$tmp/PKGBUILD" || true

if [[ $DRY_RUN -eq 1 ]]; then
  echo "== generated PKGBUILD =="; cat "$tmp/PKGBUILD"
  echo "== generated .SRCINFO =="; cat "$tmp/.SRCINFO"
  echo ">> dry run: nothing committed or pushed."
  exit 0
fi

# 5. Sync the repo copy so packaging/aur tracks what's on the AUR -------------
cp "$tmp/PKGBUILD" "$AUR_DIR/PKGBUILD"
cp "$tmp/.SRCINFO" "$AUR_DIR/.SRCINFO"

# 6. Push to the AUR ----------------------------------------------------------
echo ">> cloning AUR repo"
GIT_SSH_COMMAND='ssh -o StrictHostKeyChecking=accept-new' \
  git clone "$AUR_REMOTE" "$tmp/aur" 2>/dev/null
cp "$tmp/PKGBUILD" "$tmp/aur/PKGBUILD"
cp "$tmp/.SRCINFO" "$tmp/aur/.SRCINFO"
(
  cd "$tmp/aur"
  git add PKGBUILD .SRCINFO
  if git diff --cached --quiet; then
    echo ">> AUR already at $VERSION — nothing to push."; exit 0
  fi
  git -c user.name='John Crenshaw' -c user.email='john@jcrenshaw.dev' \
      commit -q -m "Update to $VERSION"
  GIT_SSH_COMMAND='ssh -o StrictHostKeyChecking=accept-new' git push origin master
)
echo ">> published $PKGNAME $VERSION to the AUR."
echo ">> remember to commit packaging/aur/{PKGBUILD,.SRCINFO} in the project repo."
