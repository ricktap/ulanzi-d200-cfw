#!/bin/sh
# Set the D200 Deck version in Cargo.toml, Cargo.lock, tauri.conf.json, package.json and the
# Home Assistant integration's manifest.json.
# Usage: scripts/set-version.sh 1.2.3   (a leading "v" is stripped)
set -e
V=${1#v}
echo "$V" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$' || { echo "not a semver version: $1" >&2; exit 1; }
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT/app"
# first "version = ..." line is the [package] one
awk -v v="$V" '!done && /^version = "/{print "version = \"" v "\""; done=1; next} {print}' src-tauri/Cargo.toml > src-tauri/Cargo.toml.new
mv src-tauri/Cargo.toml.new src-tauri/Cargo.toml
# the d200-deck entry in Cargo.lock
awk -v v="$V" '/^name = "d200-deck"$/{print; getline; print "version = \"" v "\""; next} {print}' src-tauri/Cargo.lock > src-tauri/Cargo.lock.new
mv src-tauri/Cargo.lock.new src-tauri/Cargo.lock
sed -i.bak "s/^  \"version\": \".*\",/  \"version\": \"$V\",/" src-tauri/tauri.conf.json package.json
# package-lock.json: the root package entry
node -e '
const fs = require("fs"), f = "package-lock.json", j = JSON.parse(fs.readFileSync(f));
j.version = j.packages[""].version = process.argv[1];
fs.writeFileSync(f, JSON.stringify(j, null, 2) + "\n");' "$V"
rm -f src-tauri/tauri.conf.json.bak package.json.bak
sed -i.bak "s/^  \"version\": \".*\"/  \"version\": \"$V\"/" "$ROOT/custom_components/d200_deck/manifest.json"
rm -f "$ROOT/custom_components/d200_deck/manifest.json.bak"
echo "version set to $V"
