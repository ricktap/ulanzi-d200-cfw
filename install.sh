#!/bin/sh
# Install or update D200 Deck on macOS from the GitHub releases.
#
#   curl -fsSL https://raw.githubusercontent.com/ricktap/ulanzi-d200-cfw/main/install.sh | sh
#   sh install.sh v0.5.0        # a given version
#
# Downloads the universal DMG of the latest release (or the given tag), puts the app in
# /Applications and starts it. curl sets no quarantine flag, so macOS opens the app
# without asking although it is not notarised. Afterwards the app updates itself.
set -eu

REPO="ricktap/ulanzi-d200-cfw"
APP="D200 Deck.app"
DEST="/Applications"
TAG="${1:-latest}"

[ "$(uname -s)" = "Darwin" ] || { echo "D200 Deck installs on macOS only." >&2; exit 1; }

if [ "$TAG" = "latest" ]; then
  API="https://api.github.com/repos/$REPO/releases/latest"
else
  API="https://api.github.com/repos/$REPO/releases/tags/$TAG"
fi

echo "Looking up $TAG release of D200 Deck..."
JSON=$(curl -fsSL -H "Accept: application/vnd.github+json" "$API") || {
  echo "No release found ($API)." >&2
  exit 1
}
# the universal DMG's download URL (no jq on a stock Mac)
URL=$(printf '%s\n' "$JSON" | grep -o '"browser_download_url": *"[^"]*_universal\.dmg"' | head -1 | sed 's/.*"\(https[^"]*\)"/\1/')
VERSION=$(printf '%s\n' "$JSON" | grep -o '"tag_name": *"[^"]*"' | head -1 | sed 's/.*"\([^"]*\)"$/\1/')
[ -n "$URL" ] || { echo "The release $VERSION has no macOS DMG." >&2; exit 1; }

TMP=$(mktemp -d)
MNT="$TMP/mnt"
cleanup() {
  hdiutil detach "$MNT" -quiet 2>/dev/null || true
  rm -rf "$TMP"
}
trap cleanup EXIT INT TERM

echo "Downloading D200 Deck $VERSION..."
curl -fL --progress-bar -o "$TMP/deck.dmg" "$URL"

mkdir -p "$MNT"
hdiutil attach "$TMP/deck.dmg" -mountpoint "$MNT" -nobrowse -readonly -quiet
[ -d "$MNT/$APP" ] || { echo "The DMG has no $APP." >&2; exit 1; }

# quit a running copy, so the new one replaces it cleanly
if pgrep -xq "D200 Deck" 2>/dev/null || pgrep -fq "$DEST/$APP/Contents/MacOS" 2>/dev/null; then
  echo "Quitting the running D200 Deck..."
  osascript -e 'tell application "D200 Deck" to quit' >/dev/null 2>&1 || true
  sleep 2
fi

SUDO=""
[ -w "$DEST" ] || SUDO="sudo"
echo "Installing to $DEST..."
$SUDO rm -rf "$DEST/$APP"
$SUDO ditto "$MNT/$APP" "$DEST/$APP"
$SUDO xattr -dr com.apple.quarantine "$DEST/$APP" 2>/dev/null || true

echo "D200 Deck $VERSION is installed. Starting it..."
open "$DEST/$APP"
